use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};

use crate::errors::{RunError, invalid_clock};

pub(crate) fn current_time() -> Result<String, RunError> {
    if cfg!(debug_assertions)
        && let Some(value) = std::env::var_os("ASSAY_TEST_FIXED_TIME")
    {
        let value = value.into_string().map_err(|_| invalid_clock())?;
        let parsed = OffsetDateTime::parse(&value, &Rfc3339).map_err(|_| invalid_clock())?;
        return parsed
            .to_offset(UtcOffset::UTC)
            .format(&Rfc3339)
            .map_err(|_| invalid_clock());
    }
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| invalid_clock())
}
