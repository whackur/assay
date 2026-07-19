use std::env;

use assay_storage::{PublicAdmissionLimits, Storage};

pub(crate) const ANONYMOUS_BUCKET_HEADER: &str = "x-assay-anonymous-bucket-id";
pub(crate) const SHARED_ANONYMOUS_BUCKET: &str =
    "488711212647543ea7c62e9193c7492ee9c97d89a24b6ae8f98ccb4efe228c96";

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) storage: Storage,
    pub(crate) admission_limits: PublicAdmissionLimits,
    pub(crate) internal_admin_token: String,
}

pub(crate) const fn bounded_secret(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256
}

pub(crate) fn required_env(name: &'static str) -> Result<String, Box<dyn std::error::Error>> {
    env::var(name).map_err(|_| format!("{name} is required").into())
}

pub(crate) fn bounded_env(
    name: &'static str,
    default: i64,
    minimum: i64,
    maximum: i64,
) -> Result<i64, Box<dyn std::error::Error>> {
    let value = match env::var(name) {
        Ok(value) => value.parse::<i64>()?,
        Err(_) => default,
    };
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("{name} must be between {minimum} and {maximum}").into());
    }
    Ok(value)
}
