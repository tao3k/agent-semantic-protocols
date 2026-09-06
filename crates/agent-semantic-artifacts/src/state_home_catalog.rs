// SPDX-FileCopyrightText: 2026 tao3k team and Contributors
//
// SPDX-License-Identifier: Apache-2.0 AND LGPL-2.1-only

//! Pure State Home catalog contract.

use serde::Deserialize;
use serde::Serialize;

use crate::ProjectBinding;
use crate::RetainedObject;
use crate::RetentionLease;
use crate::RetentionPlanner;

pub const STATE_HOME_CATALOG_SCHEMA_ID: &str = "agent.semantic-protocols.state-home-catalog";
pub const STATE_HOME_CATALOG_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CatalogGeneration(u64);

impl CatalogGeneration {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Result<Self, String> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or_else(|| "State Home catalog generation overflow".to_string())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogObservationReceipt {
    pub schema_id: String,
    pub schema_version: u32,
    pub generation: CatalogGeneration,
    pub workspace_digest: String,
    pub object_id: String,
    pub lease_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogObservation {
    pub binding: ProjectBinding,
    pub object: RetainedObject,
    pub leases: Vec<RetentionLease>,
    pub observed_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogBatchReceipt {
    pub schema_id: String,
    pub schema_version: u32,
    pub generation: CatalogGeneration,
    pub observation_count: usize,
}

pub fn admit_state_home_catalog_batch(
    current_generation: CatalogGeneration,
    observations: &[CatalogObservation],
) -> Result<CatalogBatchReceipt, String> {
    validate_state_home_catalog_observations(observations)?;
    let generation = if observations.is_empty() {
        current_generation
    } else {
        current_generation.next()?
    };
    Ok(CatalogBatchReceipt {
        schema_id: STATE_HOME_CATALOG_SCHEMA_ID.to_string(),
        schema_version: STATE_HOME_CATALOG_SCHEMA_VERSION,
        generation,
        observation_count: observations.len(),
    })
}

pub fn validate_state_home_catalog_observations(
    observations: &[CatalogObservation],
) -> Result<(), String> {
    for observation in observations {
        observation.binding.validate()?;
        if observation.object.object_id.is_empty() {
            return Err("retained object id must not be empty".to_string());
        }
        for lease in &observation.leases {
            if lease.object_id != observation.object.object_id {
                return Err(format!(
                    "retention lease {} targets a different object",
                    lease.lease_id
                ));
            }
        }
        RetentionPlanner::new(observation.object.last_observed_at_ms, 0)
            .plan(vec![observation.object.clone()], &observation.leases)
            .map(|_| ())?;
    }
    Ok(())
}
