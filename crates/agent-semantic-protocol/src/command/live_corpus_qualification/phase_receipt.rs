use std::future::Future;
use std::time::Instant;

use super::QualificationCase;

pub(super) async fn qualify_phase<T, F>(
    case: &QualificationCase,
    phase: &str,
    future: F,
) -> Result<T, String>
where
    F: Future<Output = Result<T, String>>,
{
    let started = Instant::now();
    eprintln!(
        "[live-corpus-phase] state=started phase={phase} case={} resource={} language={} provider={}",
        case.case_id, case.resource_id, case.language_id, case.provider_id
    );
    match future.await {
        Ok(value) => {
            eprintln!(
                "[live-corpus-phase] state=completed phase={phase} case={} resource={} language={} provider={} elapsedMicros={}",
                case.case_id,
                case.resource_id,
                case.language_id,
                case.provider_id,
                started.elapsed().as_micros()
            );
            Ok(value)
        }
        Err(error) => {
            let reason_kind = error
                .split_whitespace()
                .find_map(|field| field.strip_prefix("reasonKind="))
                .unwrap_or("live-corpus-phase-failed");
            eprintln!(
                "[live-corpus-phase] state=failed phase={phase} case={} resource={} language={} provider={} elapsedMicros={} reasonKind={reason_kind}",
                case.case_id,
                case.resource_id,
                case.language_id,
                case.provider_id,
                started.elapsed().as_micros()
            );
            Err(error)
        }
    }
}
