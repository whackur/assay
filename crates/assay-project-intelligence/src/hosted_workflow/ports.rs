use super::types::{
    HostedClaimedJob, HostedEvaluationAttempt, HostedEvaluationInput, HostedFailure,
    HostedPortError, HostedSourceCollection, HostedStoredSource, HostedWorkflowPolicy,
    HostedWorkflowStage,
};

pub trait HostedWorkflowStore: Sync {
    async fn claim_job(&self, worker: &str) -> Result<Option<HostedClaimedJob>, HostedPortError>;

    async fn load_evaluation_input(
        &self,
        job: &HostedClaimedJob,
    ) -> Result<Option<HostedEvaluationInput>, HostedPortError>;

    async fn store_source_collection(
        &self,
        job: &HostedClaimedJob,
        collection: &HostedSourceCollection,
    ) -> Result<HostedStoredSource, HostedPortError>;

    async fn record_evaluation_attempt(
        &self,
        job: &HostedClaimedJob,
        source: &HostedStoredSource,
        attempt: &HostedEvaluationAttempt,
    ) -> Result<(), HostedPortError>;

    async fn store_evaluation(
        &self,
        job: &HostedClaimedJob,
        source: &HostedStoredSource,
        attempt: &HostedEvaluationAttempt,
    ) -> Result<(), HostedPortError>;

    async fn settle_failure(
        &self,
        job: &HostedClaimedJob,
        stage: HostedWorkflowStage,
        failure: &HostedFailure,
        policy: HostedWorkflowPolicy,
    ) -> Result<bool, HostedPortError>;
}

pub trait HostedSourceCollectionPort: Sync {
    async fn collect(
        &self,
        job: &HostedClaimedJob,
    ) -> Result<HostedSourceCollection, HostedFailure>;
}

pub trait HostedEvaluationPort: Sync {
    async fn evaluate(
        &self,
        input: &HostedEvaluationInput,
    ) -> Result<HostedEvaluationAttempt, HostedFailure>;
}
