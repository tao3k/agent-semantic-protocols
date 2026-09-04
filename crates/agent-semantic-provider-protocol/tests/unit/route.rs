use crate::PROVIDER_ROUTE_SCHEMA_ID;
use crate::PROVIDER_ROUTE_SCHEMA_VERSION;
use crate::ProviderRouteAccess;
use crate::ProviderRouteAuthority;
use crate::ProviderRouteCache;
use crate::ProviderRouteCacheScope;
use crate::ProviderRouteCardinality;
use crate::ProviderRouteConcurrency;
use crate::ProviderRouteEffects;
use crate::ProviderRouteInputSlot;
use crate::ProviderRouteInputSource;
use crate::ProviderRouteOutput;
use crate::ProviderRouteRequiredState;
use crate::ProviderRouteRequirement;
use crate::ProviderRouteSpec;
use crate::ProviderRouteTarget;
use crate::ProviderRouteTelemetry;
use crate::ProviderRouteTelemetryPolicy;
use crate::ProviderRouteValueType;
use crate::ProviderSchemaReference;
use crate::route::require_dotted_id;
use crate::route::require_semantic_id;

fn route() -> ProviderRouteSpec {
    ProviderRouteSpec {
        schema_id: PROVIDER_ROUTE_SCHEMA_ID.into(),
        schema_version: PROVIDER_ROUTE_SCHEMA_VERSION.into(),
        route_id: "rust.search".into(),
        operation: "search".into(),
        authority: ProviderRouteAuthority::AspServer,
        target: ProviderRouteTarget {
            language_id: "rust".into(),
            provider_id: "asp-rust".into(),
        },
        request_schema: Some(ProviderSchemaReference {
            schema_id: "agent.semantic-protocols.runtime-provider-search-request".into(),
            schema_version: "1".into(),
        }),
        inputs: vec![ProviderRouteInputSlot {
            name: "query".into(),
            value_type: ProviderRouteValueType::String,
            cardinality: ProviderRouteCardinality::Required,
            source: ProviderRouteInputSource::Request,
            telemetry: ProviderRouteTelemetryPolicy::Identity,
        }],
        requirements: vec![ProviderRouteRequirement::State {
            state: ProviderRouteRequiredState::ProviderReady,
        }],
        effects: ProviderRouteEffects {
            access: ProviderRouteAccess::Read,
            idempotent: true,
            cancellable: true,
            concurrency: ProviderRouteConcurrency::SharedRead,
            streaming: false,
        },
        output: ProviderRouteOutput {
            schema: ProviderSchemaReference {
                schema_id: "agent.semantic-protocols.search-packet".into(),
                schema_version: "1".into(),
            },
            media_type: "application/json".into(),
            projection_kind: None,
        },
        failure_schema_ids: vec!["agent.semantic-protocols.route-failure".into()],
        cache: ProviderRouteCache {
            authority: ProviderRouteAuthority::AspServer,
            scope: ProviderRouteCacheScope::Workspace,
            key_slots: vec!["query".into()],
        },
        telemetry: ProviderRouteTelemetry {
            span_name: "asp.route.provider".into(),
            attribute_slots: vec!["query".into()],
        },
    }
}

#[test]
fn provider_operations_accept_single_or_dotted_semantic_ids() {
    for operation in ["query", "search", "projection-batch"] {
        require_semantic_id("operation", operation).unwrap();
    }
}

#[test]
fn route_ids_remain_globally_dotted() {
    assert!(require_dotted_id("routeId", "query").is_err());
    require_dotted_id("routeId", "rust.query").unwrap();
}

#[test]
fn compiles_a_semantic_route_without_an_argv_projection() {
    let compiled = route().compile().expect("route should compile");
    assert_eq!(compiled.spec().route_id, "rust.search");
    assert!(compiled.input_slot("query").is_some());
}

#[test]
fn rejects_unknown_cache_slots() {
    let mut route = route();
    route.cache.key_slots = vec!["missing".into()];
    assert!(
        route
            .compile()
            .unwrap_err()
            .to_string()
            .contains("unknown input slot")
    );
}

#[test]
fn rejects_telemetry_for_omitted_slots() {
    let mut route = route();
    route.inputs[0].telemetry = ProviderRouteTelemetryPolicy::Omit;
    assert!(
        route
            .compile()
            .unwrap_err()
            .to_string()
            .contains("omitted input slot")
    );
}

#[test]
fn rejects_duplicate_slots() {
    let mut route = route();
    route.inputs.push(route.inputs[0].clone());
    assert!(
        route
            .compile()
            .unwrap_err()
            .to_string()
            .contains("duplicate input slot")
    );
}
