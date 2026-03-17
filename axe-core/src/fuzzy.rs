/// Maximum number of filtered results to display.
pub const MAX_RESULTS: usize = 1000;

/// A matched result with score and highlight positions.
#[derive(Debug, Clone)]
pub struct FilteredItem {
    /// Index into the source items vector.
    pub index: usize,
    /// Match score from nucleo (higher is better).
    pub score: u32,
    /// Character positions that matched the query.
    pub match_indices: Vec<u32>,
}
