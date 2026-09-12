// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-or-later

use super::{ProviderProjectionTiming, RuntimeOwnerMaterializer};

#[test]
fn provider_lifecycle_time_is_not_runtime_engine_work() {
    let timing = ProviderProjectionTiming {
        cache_probe_micros: 7,
        provider_start_micros: 11_000,
        provider_ready_micros: 23_000,
        projection_micros: 13,
        request_wall_micros: 34_020,
    };

    assert_eq!(timing.runtime_engine_work_micros(), 20);
    assert_eq!(timing.provider_lifecycle_work_micros(), 34_000);
    assert_eq!(timing.request_wall_micros, 34_020);
}

#[test]
fn last_waiter_reclaims_the_generation_scoped_claim() {
    let materializer = RuntimeOwnerMaterializer::default();
    let first = materializer
        .claim("workspace\0generation\0owner")
        .ok()
        .expect("first owner materialization claim");
    let second = materializer
        .claim("workspace\0generation\0owner")
        .ok()
        .expect("coalesced owner materialization claim");
    assert_eq!(materializer.claims.lock().unwrap().len(), 1);

    drop(first);
    assert_eq!(materializer.claims.lock().unwrap().len(), 1);
    drop(second);
    assert!(materializer.claims.lock().unwrap().is_empty());
}
