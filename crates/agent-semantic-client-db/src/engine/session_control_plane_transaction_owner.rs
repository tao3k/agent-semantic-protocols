use super::{
    AGENT_TABLE, Arc, BTreeMap, CONTROL_PLANE_TABLE, DELEGATION_TABLE, EVENT_TABLE, Ordering, Path,
    ResidentCommittedReceipt, ResidentSessionKey, ResidentSessionLane,
    SessionControlPlaneAgentRegistration, SessionControlPlaneDelegationProposal,
    SessionControlPlaneRuntimeMetrics, SessionControlPlaneRuntimeMetricsSnapshot,
    SessionControlPlaneSnapshot, SessionControlPlaneTransactionOwner,
    SessionControlPlaneTransactionReceipt, TransactionBehavior, admit_delegation_transaction,
    connect_turso_client_db, count_rows, finish_transaction, register_agent_transaction,
    resident_replay, validate_agent_registration, validate_delegation_proposal,
};

impl SessionControlPlaneTransactionOwner {
    pub async fn open_in_client_dir(client_dir: impl AsRef<Path>) -> Result<Self, String> {
        let owner = Self {
            db_path: client_dir.as_ref().join("facts.turso"),
            resident_lanes: Arc::new(tokio::sync::RwLock::new(BTreeMap::new())),
            runtime_metrics: Arc::new(SessionControlPlaneRuntimeMetrics::default()),
        };
        owner.bootstrap().await?;
        Ok(owner)
    }

    pub async fn register_agent(
        &self,
        registration: &SessionControlPlaneAgentRegistration,
    ) -> Result<(), String> {
        validate_agent_registration(registration)?;
        let mut connection = connect_turso_client_db(&self.db_path).await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await
            .map_err(|error| {
                format!("failed to begin session control-plane transaction: {error}")
            })?;
        let result = register_agent_transaction(&transaction, registration).await;
        finish_transaction(transaction, result).await
    }

    pub async fn admit_delegation(
        &self,
        proposal: &SessionControlPlaneDelegationProposal,
    ) -> Result<SessionControlPlaneTransactionReceipt, String> {
        validate_delegation_proposal(proposal)?;
        let lane = self.resident_lane(proposal).await;
        if let Some(admission) = resident_replay(&lane, &proposal.event_id).await {
            self.runtime_metrics
                .resident_receipt_replays
                .fetch_add(1, Ordering::Relaxed);
            return Ok(SessionControlPlaneTransactionReceipt {
                admission,
                replayed: true,
            });
        }
        let _writer = lane.writer.lock().await;
        if let Some(admission) = resident_replay(&lane, &proposal.event_id).await {
            self.runtime_metrics
                .resident_receipt_replays
                .fetch_add(1, Ordering::Relaxed);
            return Ok(SessionControlPlaneTransactionReceipt {
                admission,
                replayed: true,
            });
        }
        self.runtime_metrics
            .durable_admission_transactions
            .fetch_add(1, Ordering::Relaxed);
        let mut connection = connect_turso_client_db(&self.db_path).await?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await
            .map_err(|error| {
                format!("failed to begin session control-plane transaction: {error}")
            })?;
        let result = admit_delegation_transaction(&transaction, proposal).await;
        let receipt = finish_transaction(transaction, result).await?;
        *lane.committed.write().await = Some(ResidentCommittedReceipt {
            event_id: proposal.event_id.clone(),
            admission: receipt.admission.clone(),
        });
        Ok(receipt)
    }

    pub async fn runtime_metrics(&self) -> SessionControlPlaneRuntimeMetricsSnapshot {
        SessionControlPlaneRuntimeMetricsSnapshot {
            resident_lane_count: self.resident_lanes.read().await.len() as u64,
            durable_admission_transactions: self
                .runtime_metrics
                .durable_admission_transactions
                .load(Ordering::Relaxed),
            resident_receipt_replays: self
                .runtime_metrics
                .resident_receipt_replays
                .load(Ordering::Relaxed),
            queue_capacity: 0,
            queue_depth: 0,
            queue_high_watermark: 0,
            enqueued_transitions: 0,
            completed_transitions: 0,
            drained_transitions: 0,
            cancelled_transitions: 0,
            actor_starts: 0,
            actor_stops: 0,
        }
    }

    async fn resident_lane(
        &self,
        proposal: &SessionControlPlaneDelegationProposal,
    ) -> Arc<ResidentSessionLane> {
        let key = ResidentSessionKey {
            project_id: proposal.project_id.clone(),
            root_session_id: proposal.root_session_id.clone(),
            current_session_id: proposal.current_session_id.clone(),
        };
        if let Some(lane) = self.resident_lanes.read().await.get(&key).cloned() {
            return lane;
        }
        self.resident_lanes
            .write()
            .await
            .entry(key)
            .or_default()
            .clone()
    }

    pub async fn snapshot(
        &self,
        project_id: &str,
        root_session_id: &str,
    ) -> Result<SessionControlPlaneSnapshot, String> {
        let connection = connect_turso_client_db(&self.db_path).await?;
        let mut rows = connection
            .query(
                &format!(
                    "SELECT generation, state_digest FROM {CONTROL_PLANE_TABLE} \
                     WHERE project_id = ?1 AND root_session_id = ?2"
                ),
                (project_id, root_session_id),
            )
            .await
            .map_err(|error| format!("failed to query session control-plane snapshot: {error}"))?;
        let row = rows
            .next()
            .await
            .map_err(|error| format!("failed to read session control-plane snapshot: {error}"))?
            .ok_or_else(|| "session control-plane root is not registered".to_owned())?;
        let generation = row.get::<i64>(0).map_err(|error| {
            format!("failed to decode session control-plane generation: {error}")
        })?;
        let generation = u64::try_from(generation)
            .map_err(|_| "session control-plane generation must be non-negative".to_owned())?;
        let state_digest = row
            .get::<String>(1)
            .map_err(|error| format!("failed to decode session control-plane digest: {error}"))?;

        Ok(SessionControlPlaneSnapshot {
            generation,
            state_digest,
            agent_count: count_rows(&connection, AGENT_TABLE, project_id, root_session_id).await?,
            delegation_count: count_rows(
                &connection,
                DELEGATION_TABLE,
                project_id,
                root_session_id,
            )
            .await?,
            event_count: count_rows(&connection, EVENT_TABLE, project_id, root_session_id).await?,
        })
    }

    async fn bootstrap(&self) -> Result<(), String> {
        let connection = connect_turso_client_db(&self.db_path).await?;
        for statement in [
            format!(
                "CREATE TABLE IF NOT EXISTS {CONTROL_PLANE_TABLE} (\
                 project_id TEXT NOT NULL, root_session_id TEXT NOT NULL, \
                 generation INTEGER NOT NULL, state_digest TEXT NOT NULL, \
                 PRIMARY KEY(project_id, root_session_id))"
            ),
            format!(
                "CREATE TABLE IF NOT EXISTS {AGENT_TABLE} (\
                 project_id TEXT NOT NULL, root_session_id TEXT NOT NULL, \
                 session_id TEXT NOT NULL, parent_session_id TEXT, \
                 resident_name TEXT NOT NULL, delegation_capability TEXT NOT NULL, \
                 PRIMARY KEY(project_id, root_session_id, session_id))"
            ),
            format!(
                "CREATE TABLE IF NOT EXISTS {DELEGATION_TABLE} (\
                 project_id TEXT NOT NULL, root_session_id TEXT NOT NULL, \
                 parent_session_id TEXT NOT NULL, child_session_id TEXT NOT NULL, \
                 generation INTEGER NOT NULL, \
                 PRIMARY KEY(project_id, root_session_id, parent_session_id, child_session_id))"
            ),
            format!(
                "CREATE TABLE IF NOT EXISTS {EVENT_TABLE} (\
                 project_id TEXT NOT NULL, root_session_id TEXT NOT NULL, \
                 event_id TEXT NOT NULL, receipt_json TEXT NOT NULL, \
                 observed_at_ms INTEGER NOT NULL, \
                 PRIMARY KEY(project_id, root_session_id, event_id))"
            ),
        ] {
            connection
                .execute(&statement, ())
                .await
                .map_err(|error| format!("failed to bootstrap session control plane: {error}"))?;
        }
        Ok(())
    }
}
