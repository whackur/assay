const DEFAULT_MAX_ENTRIES: usize = 250_000;
const DEFAULT_MAX_RESPONSE_BYTES: usize = 128 * 1024 * 1024;
const DEFAULT_MAX_PATH_BYTES: usize = 4_096;
const DEFAULT_MAX_BOUNDARIES: usize = 4_096;

/// Hard bounds for one recursive GitHub tree response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TreeCollectionLimits {
    max_entries: usize,
    max_response_bytes: usize,
    max_path_bytes: usize,
    max_boundaries: usize,
}

impl TreeCollectionLimits {
    /// Creates positive hard limits for tree parsing and retained metadata.
    pub fn new(
        max_entries: usize,
        max_response_bytes: usize,
        max_path_bytes: usize,
        max_boundaries: usize,
    ) -> Result<Self, &'static str> {
        if [
            max_entries,
            max_response_bytes,
            max_path_bytes,
            max_boundaries,
        ]
        .contains(&0)
        {
            return Err("tree collection limits must be positive");
        }
        Ok(Self {
            max_entries,
            max_response_bytes,
            max_path_bytes,
            max_boundaries,
        })
    }

    /// Returns the maximum number of entries processed in detail.
    pub const fn max_entries(self) -> usize {
        self.max_entries
    }

    /// Returns the maximum response bytes read.
    pub const fn max_response_bytes(self) -> usize {
        self.max_response_bytes
    }

    /// Returns the maximum bytes in one retained path.
    pub const fn max_path_bytes(self) -> usize {
        self.max_path_bytes
    }

    /// Returns the maximum retained project boundaries.
    pub const fn max_boundaries(self) -> usize {
        self.max_boundaries
    }
}

impl Default for TreeCollectionLimits {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_MAX_ENTRIES,
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            max_path_bytes: DEFAULT_MAX_PATH_BYTES,
            max_boundaries: DEFAULT_MAX_BOUNDARIES,
        }
    }
}

/// Overall availability of bounded tree collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionStatus {
    /// Every provider entry was processed within local bounds.
    Complete,
    /// Some entries or boundary facts were unavailable due to an explicit bound.
    Partial,
}

/// A reason why a successful tree collection is partial.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TreePartialReason {
    /// GitHub reported that its recursive tree response was truncated.
    ProviderTruncated,
    /// More entries existed than the configured detail bound.
    EntryLimit,
    /// At least one path exceeded the configured portable path bound.
    PathLimit,
    /// More project roots were detected than the retained boundary bound.
    BoundaryLimit,
}
