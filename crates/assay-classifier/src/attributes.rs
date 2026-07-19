//! Resolved Git Linguist attribute facts.
//!
//! Split from `lib.rs` so the attribute-availability contract stays separate
//! from path validation and policy evaluation. Git-specific parsing remains
//! outside this crate; this module only carries already-resolved facts.

/// Availability of resolved `.gitattributes` facts for one file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttributeAvailability {
    /// Attribute resolution was performed, including when neither attribute
    /// was specified.
    Available,
    /// The adapter could not resolve attributes for this file.
    Unavailable,
}

/// Resolved GitHub Linguist attributes for one file.
///
/// Git-specific parsing remains outside this crate. A Git adapter resolves
/// `.gitattributes` precedence and passes the resulting optional booleans into
/// this domain input contract. `None` means the available attribute was not
/// specified; it is distinct from unavailable attribute resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinguistAttributeFacts {
    availability: AttributeAvailability,
    generated: Option<bool>,
    vendored: Option<bool>,
}

impl LinguistAttributeFacts {
    /// Creates available facts from resolved `linguist-generated` and
    /// `linguist-vendored` values.
    pub const fn available(generated: Option<bool>, vendored: Option<bool>) -> Self {
        Self {
            availability: AttributeAvailability::Available,
            generated,
            vendored,
        }
    }

    /// Creates an explicit unavailable state without inventing false values.
    pub const fn unavailable() -> Self {
        Self {
            availability: AttributeAvailability::Unavailable,
            generated: None,
            vendored: None,
        }
    }

    /// Returns whether attribute resolution was available.
    pub const fn availability(self) -> AttributeAvailability {
        self.availability
    }

    /// Returns the resolved `linguist-generated` value when specified.
    pub const fn generated(self) -> Option<bool> {
        self.generated
    }

    /// Returns the resolved `linguist-vendored` value when specified.
    pub const fn vendored(self) -> Option<bool> {
        self.vendored
    }
}
