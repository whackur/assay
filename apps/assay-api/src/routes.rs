use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};

use crate::handlers::{live, project, ready, recent_sources, status, submit};
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
        .layer(DefaultBodyLimit::max(4 * 1024))
        .with_state(state)
}
