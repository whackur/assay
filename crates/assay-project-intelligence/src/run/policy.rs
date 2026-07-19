/// Versioned bounded-retry policy: retry counts are policy data, not constants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    pub(crate) version: &'static str,
    pub(crate) automatic_retry_budget: u32,
}

impl RetryPolicy {
    /// Returns the initial versioned bounded-retry policy.
    pub const fn v1() -> Self {
        Self {
            version: "project-run-retry-1",
            automatic_retry_budget: 2,
        }
    }

    /// Returns the versioned policy identifier folded into audit records.
    pub const fn version(&self) -> &'static str {
        self.version
    }

    /// Returns the number of automatic retries permitted after the first attempt.
    pub const fn automatic_retry_budget(&self) -> u32 {
        self.automatic_retry_budget
    }

    pub(crate) const fn max_attempts(&self) -> u32 {
        self.automatic_retry_budget + 1
    }
}
