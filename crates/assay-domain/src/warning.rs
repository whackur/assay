use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::DomainValueError;
use crate::machine_code::MachineCode;

/// Machine-readable warning code without free-form sensitive data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Warning {
    code: MachineCode,
}

impl Warning {
    pub fn new(code: &str) -> Result<Self, DomainValueError> {
        Ok(Self {
            code: MachineCode::from_str(code)?,
        })
    }

    pub fn code(&self) -> &str {
        self.code.as_str()
    }
}

/// Machine-readable limitation code without free-form sensitive data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Limitation {
    code: MachineCode,
}

impl Limitation {
    pub fn new(code: &str) -> Result<Self, DomainValueError> {
        Ok(Self {
            code: MachineCode::from_str(code)?,
        })
    }

    pub fn code(&self) -> &str {
        self.code.as_str()
    }
}