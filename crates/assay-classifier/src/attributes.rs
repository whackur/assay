//! Resolved Git Linguist attribute facts.
//!
//! Git-specific parsing stays outside this crate; this module only carries
//! already-resolved facts.

/// Availability of resolved `.gitattributes` facts for one file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttributeAvailability {
    /// Resolution was performed, including when neither attribute was specified.
    Available,
    Unavailable,
}

/// Resolved GitHub Linguist attributes for one file.
/// `None` means the available attribute was not specified; it is distinct from
/// unavailable attribute resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinguistAttributeFacts {
    availability: AttributeAvailability,
    generated: Option<bool>,
    vendored: Option<bool>,
}

impl LinguistAttributeFacts {
    pub const fn available(generated: Option<bool>, vendored: Option<bool>) -> Self {
        Self {
            availability: AttributeAvailability::Available,
            generated,
            vendored,
        }
    }

    /// Explicit unavailable state without inventing false values.
    pub const fn unavailable() -> Self {
        Self {
            availability: AttributeAvailability::Unavailable,
            generated: None,
            vendored: None,
        }
    }

    pub const fn availability(self) -> AttributeAvailability {
        self.availability
    }

    pub const fn generated(self) -> Option<bool> {
        self.generated
    }

    pub const fn vendored(self) -> Option<bool> {
        self.vendored
    }
}