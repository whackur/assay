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
    InvalidEvaluation,
    ScoreSnapshotConflict,
    PublicationNotFound,
    PublicationNotSafe,
    LeaseLost,
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(_) => formatter.write_str("hosted storage unavailable"),
            Self::InvalidEvaluation => {
                formatter.write_str("hosted evaluation violates persistence rules")
            }
            Self::ScoreSnapshotConflict => {
                formatter.write_str("hosted score snapshot conflicts with existing provenance")
            }
            Self::PublicationNotFound => formatter.write_str("publication target was not found"),
            Self::PublicationNotSafe => formatter.write_str("evaluation is not safe to publish"),
            Self::LeaseLost => formatter.write_str("job lease was reclaimed"),
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::InvalidEvaluation
            | Self::ScoreSnapshotConflict
            | Self::PublicationNotFound
            | Self::PublicationNotSafe
            | Self::LeaseLost => None,
        }
    }
}

impl From<sqlx::Error> for StorageError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}
