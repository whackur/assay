use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::DomainValueError;
use crate::machine_code::MachineCode;

/// A machine-readable warning code without free-form sensitive data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Warning {
    code: MachineCode,
}

impl Warning {
    /// Creates a warning from a canonical snake_case code.
    pub fn new(code: &str) -> Result<Self, DomainValueError> {
        Ok(Self {
            code: MachineCode::from_str(code)?,
        })
    }

    /// Returns the stable warning code.
    pub fn code(&self) -> &str {
        self.code.as_str()
    }
}

/// A machine-readable limitation code without free-form sensitive data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Limitation {
    code: MachineCode,
}

impl Limitation {
    /// Creates a limitation from a canonical snake_case code.
    pub fn new(code: &str) -> Result<Self, DomainValueError> {
        Ok(Self {
            code: MachineCode::from_str(code)?,
        })
    }

    /// Returns the stable limitation code.
    pub fn code(&self) -> &str {
        self.code.as_str()
    }
}
