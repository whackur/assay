//! Confidence expressed as integer basis points.

use crate::error::ClassificationError;

/// Confidence expressed as integer basis points from 0 through 10,000.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Confidence(u16);

impl Confidence {
    pub(crate) const CERTAIN: Self = Self(10_000);
    pub(crate) const HIGH: Self = Self(9_500);
    pub(crate) const MEDIUM_HIGH: Self = Self(8_500);
    pub(crate) const MEDIUM: Self = Self(7_500);
    pub(crate) const LOW: Self = Self(5_000);

    /// Creates confidence from integer basis points in the inclusive range
    /// 0 through 10,000.
    pub fn try_from_basis_points(value: u16) -> Result<Self, ClassificationError> {
        if value > 10_000 {
            return Err(ClassificationError::confidence(
                "expected at most 10000 basis points",
            ));
        }
        Ok(Self(value))
    }

    /// Returns confidence in basis points, where 10,000 is 1.0.
    pub const fn basis_points(self) -> u16 {
        self.0
    }
}
