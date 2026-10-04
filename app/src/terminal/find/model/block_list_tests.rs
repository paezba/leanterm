use super::{BlockListFindRun, BlockListMatch};

impl BlockListFindRun {
    fn all_matches(&self) -> &[BlockListMatch] {
        &self.matches[..]
    }
}
