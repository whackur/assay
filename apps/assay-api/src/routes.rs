use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};

use crate::handlers::{
    ai_analysis, approve_ai_analysis, live, project, ready, recent_sources, review_queue, status,
    submit,
};
use crate::state::AppState;

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/internal/hosted/recent-sources", get(recent_sources))
        .route("/internal/hosted/submissions", post(submit))
        .route("/internal/hosted/submissions/{id}", get(status))
        .route(
            "/internal/hosted/projects/github/{owner}/{repository}",
            get(project),
        )
        .route(
            "/api/v1/projects/github/{owner}/{repository}/ai-analysis",
            get(ai_analysis),
        )
        .route(
            "/internal/admin/hosted/ai-analysis/approve",
            post(approve_ai_analysis),
        )
        .route(
            "/internal/admin/hosted/ai-analysis/review-queue",
            get(review_queue),
        )
        .layer(DefaultBodyLimit::max(4 * 1024))
        .with_state(state)
}
