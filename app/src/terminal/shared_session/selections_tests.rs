use warpui::text::SelectionType;

use crate::terminal::model::blocks::BlockListPoint;
use crate::terminal::model::index::{Point, Side};
use crate::terminal::model::terminal_model::WithinBlock;
use crate::terminal::TerminalModel;

/// Creates a [`SelectionType::Simple`], left-to-right text selection
/// from `start` to `end` in the `model`'s blocklist.
fn create_simple_text_selection(
    model: &mut TerminalModel,
    start: WithinBlock<Point>,
    end: WithinBlock<Point>,
) {
    let start_block_point = BlockListPoint::from_within_block_point(&start, model.block_list());
    let end_block_point = BlockListPoint::from_within_block_point(&end, model.block_list());
    model
        .block_list_mut()
        .start_selection(start_block_point, SelectionType::Simple, Side::Left);
    model
        .block_list_mut()
        .update_selection(end_block_point, Side::Right);
}





