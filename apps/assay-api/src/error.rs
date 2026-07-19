use assay_project_intelligence::HostedErrorResponse;
use assay_storage::AdmissionError;
use axum::{
    Json,
    http::{HeaderValue, StatusCode, header::RETRY_AFTER},
    response::{IntoResponse, Response},
};

pub(crate) struct ApiError {
    status: StatusCode,
    code: &'static str,
    retry_after_seconds: Option<i64>,
}

impl ApiError {
    pub(crate) fn bad_request(code: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            retry_after_seconds: None,
        }
    }

    pub(crate) fn not_found(code: &'static str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code,
            retry_after_seconds: None,
        }
    }

    pub(crate) fn service_unavailable(code: &'static str) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code,
            retry_after_seconds: None,
        }
    }

    pub(crate) fn database(_: sqlx::Error) -> Self {
        Self::service_unavailable("database_unavailable")
    }

    pub(crate) fn admission(error: AdmissionError) -> Self {
        match error {
            AdmissionError::CapacityFull => Self {
                status: StatusCode::TOO_MANY_REQUESTS,
                code: "analysis_capacity_full",
                retry_after_seconds: None,
            },
            AdmissionError::RateLimited {
                scope,
                retry_after_seconds,
            } => Self {
                status: StatusCode::TOO_MANY_REQUESTS,
                code: match scope {
                    "anonymous_client" => "anonymous_submission_rate_limited",
                    "repository_owner" => "repository_owner_rate_limited",
                    _ => "provider_capacity_rate_limited",
                },
                retry_after_seconds: Some(retry_after_seconds),
            },
            AdmissionError::Database(_) => Self::service_unavailable("database_unavailable"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status, Json(HostedErrorResponse::new(self.code))).into_response();
        if let Some(seconds) = self.retry_after_seconds
            && let Ok(value) = HeaderValue::from_str(&seconds.max(1).to_string())
        {
            response.headers_mut().insert(RETRY_AFTER, value);
        }
        response
    }
}
