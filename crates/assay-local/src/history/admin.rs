//! Local-operator capability token.
//!
//! Mutating history (soft deletion, restoration, purge) requires presenting a
//! [`LocalAdministrator`], so those journal operations cannot happen without
//! operator authority.

/// Capability held only by the single local operator. Mutating history
/// requires presenting this token, so soft deletion, restoration, and purge
/// cannot happen without operator authority.
#[derive(Clone, Copy, Debug)]
pub struct LocalAdministrator(());

impl LocalAdministrator {
    pub const fn assume_local_operator() -> Self {
        Self(())
    }
}
