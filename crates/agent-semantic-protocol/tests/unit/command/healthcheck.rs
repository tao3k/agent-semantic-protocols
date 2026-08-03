use super::{
    CatalogReadinessReceipt, collect_catalog_readiness, healthcheck_command_result, overall_status,
};

#[test]
fn catalog_readiness_success_preserves_read_receipt_without_issue() {
    let mut issues = Vec::new();
    let receipt = collect_catalog_readiness(
        &mut issues,
        Ok(
            super::super::global_provider_catalog::GlobalProviderCatalogReadiness {
                catalog_generation: "generation-test".to_owned(),
                provider_count: 7,
                elapsed_micros: 41,
            },
        ),
    );

    assert!(issues.is_empty());
    assert!(matches!(
        receipt,
        CatalogReadinessReceipt::Ready {
            catalog_generation,
            provider_count: 7,
            elapsed_micros: 41,
        } if catalog_generation == "generation-test"
    ));
}

#[test]
fn catalog_readiness_failure_adds_exact_error_issue() {
    let mut issues = Vec::new();
    let message = "content store publish: Operation not permitted".to_owned();
    let receipt = collect_catalog_readiness(&mut issues, Err(message.clone()));

    assert!(matches!(
        receipt,
        CatalogReadinessReceipt::Error { message: actual } if actual == message
    ));
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].severity, "error");
    assert_eq!(issues[0].code, "global-provider-catalog-readiness-failed");
    assert_eq!(issues[0].message, message);
    assert_eq!(overall_status(&issues), "error");
}

#[test]
fn healthcheck_error_status_returns_nonzero_contract() {
    assert_eq!(
        healthcheck_command_result("error"),
        Err("healthcheck status=error".to_owned())
    );
    assert_eq!(healthcheck_command_result("degraded"), Ok(()));
    assert_eq!(healthcheck_command_result("ok"), Ok(()));
}

#[test]
fn healthcheck_reuses_resolved_context_before_repairing_generation_locator() {
    let source = include_str!("../../../src/command/healthcheck.rs");
    let session_contract_start = source
        .find("runtime_server_workspace_session_for_resolved_admission_async")
        .expect("healthcheck must use the resolved admission session constructor");
    let session_contract_end = (session_contract_start + 500).min(source.len());
    let session_contract = &source[session_contract_start..session_contract_end];
    let resolved_state = source
        .find("ResolvedState::resolve(context.cwd())")
        .expect("healthcheck must resolve workspace state once");
    let workspace_timer = source
        .find("let workspace_generation_started")
        .expect("healthcheck must measure workspace generation latency");

    assert!(source.contains("runtime_server_workspace_session_for_resolved_admission_async"));
    assert!(
        session_contract.contains("resolved_state.workspace.workspace_id.to_string()"),
        "healthcheck session contract:\n{session_contract}"
    );
    assert!(session_contract.contains("&resolved_state.workspace.root"));
    assert!(
        resolved_state < workspace_timer,
        "workspace state resolution must remain outside the measured hot path"
    );
    assert!(source.contains("session.repair_runtime_generation_locator().await"));
    assert!(source.contains("workspace_generation_started.elapsed().as_micros()"));
    assert!(source.contains("elapsedMicros={} error={}"));
    assert!(!source.contains(
        "runtime_server_workspace_session_for_admission_async(\n                        context.cwd()"
    ));
    assert!(!source.contains("wait_terminal"));
}
