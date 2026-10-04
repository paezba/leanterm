use std::collections::HashMap;

use itertools::Itertools;
use warpui::App;

use super::{BlockListFindRun, BlockListMatch};
use crate::terminal::block_filter::BlockFilterQuery;
use crate::terminal::find::BlockGridMatch;
use crate::terminal::find::model::FindOptions;
use crate::terminal::find::model::block_list::run_find_on_block_list;
use crate::terminal::model::index::Point;
use crate::terminal::model::terminal_model::{BlockIndex, BlockSortDirection};
use crate::terminal::{GridType, TerminalModel};
use crate::view_components::find::FindDirection;

impl BlockListFindRun {
    fn all_matches(&self) -> &[BlockListMatch] {
        &self.matches[..]
    }
}







