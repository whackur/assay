use std::{error::Error, fmt};

#[derive(Debug)]
pub enum AdmissionError {
    Database(sqlx::Error),
    CapacityFull,
    RateLimited {
        scope: &'static str,
        retry_after_seconds: i64,
    },
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(_) => formatter.write_str("admission storage unavailable"),
            Self::CapacityFull => formatter.write_str("analysis capacity is full"),
            Self::RateLimited { scope, .. } => {
                write!(formatter, "{scope} admission is cooling down")
            }
        }
    }
}

impl Error for AdmissionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::CapacityFull | Self::RateLimited { .. } => None,
        }
    }
}

impl From<sqlx::Error> for AdmissionError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

#[derive(Debug)]
pub enum StorageError {
    Database(sqlx::Error),
    LeaseLost,
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(_) => formatter.write_str("hosted storage unavailable"),
            Self::LeaseLost => formatter.write_str("job lease was reclaimed"),
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::LeaseLost => None,
        }
    }
}

impl From<sqlx::Error> for StorageError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}
