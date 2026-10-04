use crate::terminal::model::block::SerializedBlockListItem;
use float_cmp::{approx_eq, assert_approx_eq};
use parking_lot::FairMutex;
use warp_core::features::FeatureFlag;
use warpui::App;
use warpui::elements::DEFAULT_UI_LINE_HEIGHT_RATIO;
use warpui::units::IntoLines;

use super::*;
use crate::settings::TerminalSpacing;
use crate::terminal::event::Event;
use crate::terminal::model::ansi::Handler;
use crate::terminal::model::test_utils::TestBlockListBuilder;
use crate::terminal::model::{TerminalModel, test_utils};
use crate::terminal::view::{InlineBannerItem, InlineBannerType};
use crate::terminal::{BlockListSettings, SizeUpdateReason};

pub fn input_string(block_list: &mut BlockList, input: &str) {
    for c in input.chars() {
        block_list.input(c);
    }
}

// Returns a block list in the PostBootstrapPrecmd stage. Use this to perform tests
// about the block list, and disregard the behavior of the hidden bootstrapping block.
pub fn new_bootstrapped_block_list(
    block_sizes_override: Option<BlockSize>,
    honor_ps1_override: Option<bool>,
    channel_event_proxy: ChannelEventListener,
) -> BlockList {
    let mut builder = TestBlockListBuilder::new().with_channel_event_proxy(channel_event_proxy);
    if let Some(honor_ps1) = honor_ps1_override {
        builder = builder.with_honor_ps1(honor_ps1);
    }
    if let Some(block_sizes) = block_sizes_override {
        builder = builder.with_block_sizes(block_sizes);
    }

    let mut block_list = builder.build();
    advance_to_bootstrapped(&mut block_list, Default::default());

    assert_eq!(block_list.blocks().len(), 3);
    block_list
}

// Helper function to create dummy blocks
pub fn insert_block(block_list: &mut BlockList, command: &str, output: &str) -> BlockIndex {
    // Create a block.
    block_list.start_active_block();

    let block_index = block_list.active_block_index();

    // Fill the command grid.  This logic splits on newlines, invoking
    // `linefeed()` only when a `\n` character actually appears in the input
    // string.
    let mut lines = command.split('\n');
    if let Some(line) = lines.next() {
        input_string(block_list, line);
    }
    for line in lines {
        block_list.carriage_return();
        block_list.linefeed();
        input_string(block_list, line);
    }
    block_list.preexec(Default::default());

    // Fill the output grid.  This logic splits on newlines, invoking
    // `linefeed()` only when a `\n` character actually appears in the input
    // string.
    let mut lines = output.split('\n');
    if let Some(line) = lines.next() {
        input_string(block_list, line);
    }
    for line in lines {
        block_list.carriage_return();
        block_list.linefeed();
        input_string(block_list, line);
    }
    command_finished_and_precmd(block_list);

    block_index
}

// Helper function to create dummy blocks, with custom prompts.
pub fn insert_block_with_prompt(
    block_list: &mut BlockList,
    prompt: &str,
    command: &str,
    output: &str,
) -> BlockIndex {
    block_list.prompt_only_precmd(PromptMetadata {
        ps1: Some(hex::encode(prompt)),
        honor_ps1: Some(true),
        ..Default::default()
    });

    block_list.prompt_marker(ansi::PromptMarker::StartPrompt {
        kind: ansi::PromptKind::Initial,
    });
    // Fill the prompt grid.  This logic splits on newlines, adding a
    // CR/LF only when a `\n` character actually appears in the input
    // string.
    let mut lines = prompt.split('\n');
    if let Some(line) = lines.next() {
        input_string(block_list, line);
    }
    for line in lines {
        block_list.carriage_return();
        block_list.linefeed();
        input_string(block_list, line);
    }
    block_list.prompt_marker(ansi::PromptMarker::EndPrompt);

    insert_block(block_list, command, output)
}

/// Calling `command_finished` is all that's necessary for tests that only
/// advance the block list and check the state (e.g. like the length of the
/// block list, the bootstrapped state). Tests that check for messages sent to the
/// view need to also call `precmd_with_completion_metadata`.
pub fn command_finished_and_precmd(block_list: &mut BlockList) {
    let completion_metadata = ansi::CompletionMetadata::default();
    block_list.command_finished(CommandFinishedValue {
        completion_metadata: completion_metadata.clone(),
        ..Default::default()
    });
    block_list.precmd_with_completion_metadata(PrecmdValue {
        completion_metadata,
        prompt_metadata: PromptMetadata::default(),
    });
}

#[test]
fn classifies_next_block_ids_relative_to_the_active_block() {
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());
    let previous_active_id = block_list.active_block_id().clone();
    let next_block_id = BlockId::new();

    assert_eq!(
        block_list.classify_next_block_id(&previous_active_id),
        NextBlockIdDisposition::ActiveDuplicate
    );
    assert_eq!(
        block_list.classify_next_block_id(&next_block_id),
        NextBlockIdDisposition::Novel
    );

    block_list.complete_active_block_and_advance(ansi::CompletionMetadata {
        exit_code: 0.into(),
        next_block_id: next_block_id.clone(),
    });

    assert_eq!(
        block_list.classify_next_block_id(&previous_active_id),
        NextBlockIdDisposition::ExistingCollision
    );
    assert_eq!(
        block_list.classify_next_block_id(&next_block_id),
        NextBlockIdDisposition::ActiveDuplicate
    );
}
fn drain_terminal_events(events_rx: &async_channel::Receiver<Event>) -> Vec<Event> {
    let mut events = Vec::new();
    while let Ok(event) = events_rx.try_recv() {
        events.push(event);
    }
    events
}

/// Advances the block list to the ScriptExecution stage.
fn advance_to_script_execution(block_list: &mut BlockList) {
    assert!(
        block_list.bootstrap_stage == BootstrapStage::RestoreBlocks
            || block_list.bootstrap_stage == BootstrapStage::WarpInput,
        "Unexpected bootstrap stage: {:?}",
        block_list.bootstrap_stage
    );

    command_finished_and_precmd(block_list);
    assert_eq!(block_list.bootstrap_stage, BootstrapStage::ScriptExecution);
}

/// Advances the block list through bootstrapping (to the PostBootstrapPrecmd
/// stage).
fn advance_to_bootstrapped(block_list: &mut BlockList, data: BootstrappedValue) {
    if block_list.bootstrap_stage == BootstrapStage::RestoreBlocks
        || block_list.bootstrap_stage == BootstrapStage::WarpInput
    {
        advance_to_script_execution(block_list);
    }

    block_list.bootstrapped(data);

    command_finished_and_precmd(block_list);
    assert_eq!(
        block_list.bootstrap_stage,
        BootstrapStage::PostBootstrapPrecmd
    );
}





#[test]
fn test_iterm_image_early_output_routes_to_background_block() {
    let _iterm_images = FeatureFlag::ITermImages.override_enabled(true);
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());
    let blocks_before = block_list.blocks.len();

    assert!(block_list.is_early_output());

    block_list.handle_completed_iterm_image(test_utils::test_iterm_image(2));

    assert_eq!(block_list.blocks.len(), blocks_before + 1);
    let background_block = &block_list.blocks[block_list.blocks.len() - 2];
    assert!(background_block.is_background());
    assert!(!background_block.output_grid().is_empty());
    assert!(!block_list.active_block().started());
}

#[test]
fn test_kitty_image_early_output_routes_to_background_block() {
    let _kitty_images = FeatureFlag::KittyImages.override_enabled(true);
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());
    let mut metadata = test_utils::test_kitty_image_metadata_map(2);
    let blocks_before = block_list.blocks.len();

    assert!(block_list.is_early_output());

    block_list
        .handle_completed_kitty_action(
            test_utils::test_kitty_store_and_display_action(2, 1),
            &mut metadata,
        )
        .expect("kitty action should be handled")
        .expect("kitty action should render");

    assert_eq!(block_list.blocks.len(), blocks_before + 1);
    let background_block = &block_list.blocks[block_list.blocks.len() - 2];
    assert!(background_block.is_background());
    assert!(!background_block.output_grid().is_empty());
    assert!(!block_list.active_block().started());
}

#[test]
fn test_kitty_store_only_early_output_does_not_create_background_block() {
    let _kitty_images = FeatureFlag::KittyImages.override_enabled(true);
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());
    let mut metadata = test_utils::test_kitty_image_metadata_map(2);
    let blocks_before = block_list.blocks.len();

    assert!(block_list.is_early_output());

    block_list
        .handle_completed_kitty_action(test_utils::test_kitty_store_only_action(2), &mut metadata)
        .expect("kitty action should be handled")
        .expect("kitty action should store");

    assert_eq!(block_list.blocks.len(), blocks_before);
    assert!(!block_list.active_block().started());
}

#[test]
fn test_zero_sized_kitty_early_output_does_not_create_background_block() {
    let _kitty_images = FeatureFlag::KittyImages.override_enabled(true);
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());
    let mut metadata = test_utils::test_kitty_image_metadata_map(2);
    let mut action = test_utils::test_kitty_store_and_display_action(2, 1);
    let blocks_before = block_list.blocks.len();

    if let KittyAction::StoreAndDisplay(action) = &mut action {
        action.placement_data.cols = Some(0);
    }

    assert!(block_list.is_early_output());

    block_list
        .handle_completed_kitty_action(action, &mut metadata)
        .expect("kitty action should be handled")
        .expect("kitty action should be ignored");

    assert_eq!(block_list.blocks.len(), blocks_before);
    assert!(!block_list.active_block().started());
}

// This test covers the case where sometimes sumtree could have inconsistency
// where its internal node holds a larger summary than all its children nodes' summary combined
// due to floating point precision error. SumTree should be able to handle this case
// and place the cursor in the right leaf node.
#[test]
fn test_cursor_seeking_in_sumtree_with_floating_point_inconsistency() {
    let mut tree = SumTree::<BlockHeightItem>::new();
    // Heights from an actual error state.
    let heights: Vec<f32> = vec![
        873.19, 0.0, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19,
        4.19, 4.19, 4.19, 5.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19,
        4.19, 4.19, 4.19, 4.19, 5.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 4.19, 0.0,
    ];

    let items: Vec<BlockHeightItem> = heights
        .iter()
        .cloned()
        .map(|height| BlockHeightItem::Block(height.into()))
        .collect();

    for item in items {
        tree.push(item);
    }

    // Sum of all the heights.  We use this instead of a manually-computed sum
    // because the IEEE 754 representation of the individual block heights is
    // not exactly the same as the decimal representation, and so we can't use
    // 1046.98 as the total height (the sum of the heights in decimal).
    let total_sum = tree.extent::<BlockHeight>().0;

    let mut cursor = tree.cursor::<BlockHeight, BlockHeightSummary>();
    // Seeking at total sum with bias to the right should put the cursor in the end.
    cursor.seek_clamped(&BlockHeight::from(total_sum), SeekBias::Right);
    assert!(cursor.item().is_none());

    // Seeking at total sum with bias to the left should put the cursor at the last non-zero item.
    cursor.seek_clamped(&BlockHeight::from(total_sum), SeekBias::Left);
    assert_lines_approx_eq!(cursor.item().unwrap().height().into_lines(), 4.19);

    // Seeking at a smaller sum should still work. It should put the cursor at the last non-zero item.
    cursor.seek_clamped(&BlockHeight::from(1046.9797), SeekBias::Right);
    assert_lines_approx_eq!(cursor.item().unwrap().height().into_lines(), 4.19);
}

#[test]
fn test_internal_consistency_of_block_height_summing() {
    let mut tree = SumTree::<BlockHeightItem>::new();
    // Heights taken from an actual crash
    let heights: Vec<f32> = vec![
        7.69, 458.69, 5.69, 7.69, 7.69, 40.69, 944.69, 641.69, 116.69, 65.69, 143.69, 1.5, 0., 0.,
        45.0, 0.,
    ];
    let items: Vec<BlockHeightItem> = heights
        .iter()
        .cloned()
        .map(|height| BlockHeightItem::Block(height.into()))
        .collect();
    tree.extend(items);

    let mut cursor = tree.cursor::<BlockHeight, BlockHeightSummary>();
    // Should seek to between elements 11 and 12
    cursor.seek(&BlockHeight::from(2442.0898), SeekBias::Right);
    // Getting the item here should not cause a crash and should return the correct item.
    assert_lines_approx_eq!(cursor.item().unwrap().height().into_lines(), 1.5);

    let mut cursor = tree.cursor::<BlockHeight, BlockHeightSummary>();
    // Increasing the seek position slightly should seek to between elements 13 and 14
    cursor.seek(&BlockHeight::from(2442.1), SeekBias::Right);
    // Getting the item here should not cause a crash and should return the correct item.
    assert_lines_approx_eq!(cursor.item().unwrap().height().into_lines(), 45.);
}

#[test]
fn test_update_padding_block_heights() {
    App::test((), |app| async move {
        app.add_singleton_model(BlockListSettings::new_with_defaults);
        let mut block_list =
            new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());

        // Create two blocks, each with 3 command lines and 3 output lines.
        for _ in 0..2 {
            insert_block(&mut block_list, "foo\nbar\nbazz", "foo\nbar\nbazz");
        }

        let current_block_height = block_list.block_heights().summary().height;

        let spacing = app.read(|ctx| TerminalSpacing::compact(DEFAULT_UI_LINE_HEIGHT_RATIO, ctx));
        block_list
            .update_blockheight_items(spacing.block_padding, spacing.subshell_separator_height);

        let new_block_height = block_list.block_heights().summary().height;
        assert!(!approx_eq!(Lines, current_block_height, new_block_height));
    });
}

// Disabled because it's flaky on CI.
// #[test]
// pub fn test_clear_visible_screen() {
//     let mut block_list = new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());

//     // Create two blocks, each with 3 command lines and 3 output lines.
//     for _ in 0..2 {
//         insert_block(&mut block_list, "foo\nbar\nbazz\n", "foo\nbar\nbazz\n");
//     }

//     // Three from the bootstrapped block list, plus two calls to `block_finished`.
//     assert_eq!(block_list.blocks.len(), 5);

//     assert_float_eq!(block_list.blocks[0].height(), 0.);
//     assert_float_eq!(block_list.blocks[1].height(), 0.);
//     assert_float_eq!(block_list.blocks[2].height(), 8.5);
//     assert_float_eq!(block_list.blocks[3].height(), 8.5);
//     assert_float_eq!(block_list.blocks[4].height(), 0.);

//     assert_lines_approx_eq!(block_list.block_heights.summary().height, 17.);
//     block_list.set_next_gap_height_in_lines(17.0.into_lines());

//     // Now clear the visible screen--the number of blocks shouldn't change but total height
//     // should increase by the size of the visible screen (10).
//     block_list.clear_visible_screen();

//     assert_eq!(block_list.blocks.len(), 5);
//     assert_lines_approx_eq!(block_list.block_heights.summary().height, 34.);

//     // The active block should be after the gap within the sumtree.
//     assert_eq!(block_list.block_heights.summary().total_count, 6);
//     assert_eq!(block_list.active_gap.as_ref().unwrap().index, 4);
//     assert_lines_approx_eq!(block_list.active_gap.as_ref().unwrap().current_height, 17.);

//     // Update the height of the active block to now be 5 lines--the active gap should shrink.
//     block_list.start_active_block();
//     input_string(&mut block_list, "foo");
//     block_list.linefeed();
//     input_string(&mut block_list, "bar");
//     block_list.linefeed();
//     input_string(&mut block_list, "bazz");

//     assert_float_eq!(block_list.blocks[4].height(), 5.);
//     assert_lines_approx_eq!(block_list.block_heights.summary().height, 34.);
//     assert_lines_approx_eq!(block_list.active_gap.as_ref().unwrap().current_height, 12.);

//     // Clear the screen again--ensure there's still only one gap that is reset.
//     block_list.clear_visible_screen();
//     assert_lines_approx_eq!(block_list.block_heights.summary().height, 39.);
//     assert_lines_approx_eq!(block_list.active_gap.as_ref().unwrap().current_height, 17.);
//     assert_eq!(block_list.active_gap.as_ref().unwrap().index, 5);

//     // Add a new block with many lines--the active gap should no longer exist.
//     block_list.active_block_mut().finish(0);
//     block_list.update_active_block_height();

//     command_finished_and_precmd(&mut block_list);
//     block_list.start_active_block();
//     for _ in 0..20 {
//         input_string(&mut block_list, "foo");
//         block_list.linefeed();
//     }

//     assert_lines_approx_eq!(block_list.block_heights.summary().height, 44.0);
//     assert!(block_list.active_gap.is_none());
// }


// Add a few restored blocks and ensure they show up appropriately.




// Bootstrap with no restored blocks and no script execution.
// There will be a special hidden InitShell block and everything else should be empty.


#[test]
pub fn test_insert_non_block_item() {
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());

    // Create two blocks, each with 3 command lines and 3 output lines.
    let first_block_index = insert_block(&mut block_list, "foo\nbar\nbazz\n", "foo\nbar\nbazz\n");
    assert_eq!(first_block_index, BlockIndex(2));
    assert_eq!(first_block_index.to_total_index(&block_list), TotalIndex(2));

    insert_block(&mut block_list, "foo\nbar\nbazz\n", "foo\nbar\nbazz\n");

    // This happens to be the block height of such blocks ^.
    let block_height = 8.5;

    // Add a non-block item at the start of the block list (after the hidden blocks).
    let inserted_index = block_list.insert_non_block_item_before_block(
        first_block_index,
        BlockHeightItem::RestoredBlockSeparator {
            height_when_visible: BlockHeight::from(RESTORED_BLOCK_SEPARATOR_HEIGHT),
            is_hidden: false,
        },
    );
    assert_eq!(inserted_index, TotalIndex(2));

    // Add a non-block item at the end of the block list.
    let inserted_index = block_list.insert_non_block_item_before_block(
        block_list.active_block_index(),
        BlockHeightItem::RestoredBlockSeparator {
            height_when_visible: BlockHeight::from(RESTORED_BLOCK_SEPARATOR_HEIGHT),
            is_hidden: false,
        },
    );
    assert_eq!(inserted_index, TotalIndex(5));

    // The blocks should remain unchanged.
    assert_eq!(block_list.blocks.len(), 5);
    assert_lines_approx_eq!(
        block_list.blocks[0].height(&crate::terminal::model::block::TranscriptScope::Terminal),
        0.
    );
    assert_lines_approx_eq!(
        block_list.blocks[1].height(&crate::terminal::model::block::TranscriptScope::Terminal),
        0.
    );
    assert_lines_approx_eq!(
        block_list.blocks[2].height(&crate::terminal::model::block::TranscriptScope::Terminal),
        block_height
    );
    assert_lines_approx_eq!(
        block_list.blocks[3].height(&crate::terminal::model::block::TranscriptScope::Terminal),
        block_height
    );
    assert_lines_approx_eq!(
        block_list.blocks[4].height(&crate::terminal::model::block::TranscriptScope::Terminal),
        0.
    );

    fn assert_block_height_summary_eq(a: BlockHeightSummary, b: BlockHeightSummary) {
        assert_eq!(a.block_count, b.block_count);
        assert_eq!(a.total_count, b.total_count);
        assert_lines_approx_eq!(a.height, b.height);
    }

    // But the block heights (which encapsulates blocks + nonblocks) should reflect the new items.

    let summaries: Vec<BlockHeightSummary> = block_list
        .block_heights()
        .items()
        .iter()
        .map(|i| i.summary())
        .collect();

    // 2 hidden blocks + non-block + 2 blocks + non-block + active block
    assert_eq!(block_list.block_heights().summary().total_count, 7);

    // The first two items are hidden blocks.
    assert_block_height_summary_eq(
        summaries[0],
        BlockHeightSummary {
            total_count: 1,
            block_count: 1,
            height: Lines::zero(),
        },
    );
    assert_block_height_summary_eq(
        summaries[1],
        BlockHeightSummary {
            total_count: 1,
            block_count: 1,
            height: Lines::zero(),
        },
    );

    // The next item should be the non-block item.
    assert_block_height_summary_eq(
        summaries[2],
        BlockHeightSummary {
            total_count: 1,
            block_count: 0,
            height: RESTORED_BLOCK_SEPARATOR_HEIGHT.into_lines(),
        },
    );

    // The next two items are the blocks.
    assert_block_height_summary_eq(
        summaries[3],
        BlockHeightSummary {
            total_count: 1,
            block_count: 1,
            height: block_height.into_lines(),
        },
    );
    assert_block_height_summary_eq(
        summaries[4],
        BlockHeightSummary {
            total_count: 1,
            block_count: 1,
            height: block_height.into_lines(),
        },
    );

    // The next item should be the non-block item.
    assert_block_height_summary_eq(
        summaries[5],
        BlockHeightSummary {
            total_count: 1,
            block_count: 0,
            height: RESTORED_BLOCK_SEPARATOR_HEIGHT.into_lines(),
        },
    );

    // The last item is the active block.
    assert_block_height_summary_eq(
        summaries[6],
        BlockHeightSummary {
            total_count: 1,
            block_count: 1,
            height: Lines::zero(),
        },
    );

    // Overall, we have two blocks with 8.5 height and two separator with 1.5 height.
    let total_height = 2. * block_height + 2. * RESTORED_BLOCK_SEPARATOR_HEIGHT;
    assert_lines_approx_eq!(block_list.block_heights.summary().height, total_height);

    // Now clear the visible screen--the number of blocks shouldn't change but total height
    // should increase by the size of the visible screen.
    block_list.set_next_gap_height_in_lines(total_height.into_lines());
    block_list.clear_visible_screen();

    assert_eq!(block_list.blocks.len(), 5);
    assert_eq!(block_list.block_heights().summary().total_count, 8);
    assert_lines_approx_eq!(
        block_list.block_heights.summary().height,
        total_height
            + block_list
                .next_gap_height()
                .expect("gap height should be set")
                .as_f64()
    );

    // The active block should be after the gap within the sumtree.
    assert_eq!(block_list.active_gap.as_ref().unwrap().index, 6);
    assert_lines_approx_eq!(
        block_list.active_gap.as_ref().unwrap().current_height,
        total_height
    );
}


#[test]
fn test_matching_block_by_index() {
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());

    // Add a non-hidden command block, and then a background block.
    insert_block(&mut block_list, "command\n", "output\n");
    input_string(&mut block_list, "background output");

    // The default filter should include background blocks.
    assert_eq!(
        block_list.last_matching_block_by_index(BlockFilter::default()),
        Some(3.into())
    );
    assert_eq!(block_list.last_non_hidden_block_by_index(), Some(3.into()));

    // The command-only filter should exclude background blocks.
    assert_eq!(
        block_list.last_matching_block_by_index(BlockFilter::commands()),
        Some(2.into())
    );

    // It should be possible to include hidden blocks.
    assert_eq!(
        block_list.first_matching_block_by_index(BlockFilter {
            include_hidden: true,
            ..Default::default()
        }),
        Some(0.into())
    );
}

#[test]
fn test_banner_insertion_and_removal() {
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());

    // Create the following blocklist:
    // block -> banner -> block -> banner -> block -> banner
    let first_block_index = insert_block(&mut block_list, "1", "1");
    insert_block(&mut block_list, "2", "2");
    let last_block_index = insert_block(&mut block_list, "3", "3");

    block_list.insert_inline_banner_after_block(
        first_block_index,
        InlineBannerItem::new(0, InlineBannerType::NotificationsDiscovery),
    );
    block_list.insert_inline_banner_before_block(
        last_block_index,
        InlineBannerItem::new(1, InlineBannerType::NotificationsDiscovery),
        None,
    );
    block_list.append_inline_banner(InlineBannerItem::new(
        2,
        InlineBannerType::NotificationsDiscovery,
    ));

    // Three inserted blocks + three banners + three blocks from bootstrapping
    // Note that in the expected_total_height calculations, the active block
    // has a height of 0 since it hasn't hit preexec
    let total_block_count_after_insertion = 6;
    let total_count_after_insertion = 9;
    assert_eq!(
        block_list.block_heights.summary().block_count,
        total_block_count_after_insertion
    );
    assert_eq!(
        block_list.block_heights.summary().total_count,
        total_count_after_insertion
    );

    let expected_total_height = (block_list.blocks[2]
        .height(&crate::terminal::model::block::TranscriptScope::Terminal)
        .as_f64()
        * 3.
        + 3. * INLINE_BANNER_HEIGHT)
        .into_lines();
    assert_lines_approx_eq!(
        block_list.block_heights.summary().height,
        expected_total_height
    );

    // Remove the first banner
    block_list.remove_inline_banner(0);
    assert_eq!(
        block_list.block_heights.summary().block_count,
        total_block_count_after_insertion
    );
    assert_eq!(
        block_list.block_heights.summary().total_count,
        total_count_after_insertion - 1
    );

    let expected_total_height_after_removal = expected_total_height - INLINE_BANNER_HEIGHT;
    assert_lines_approx_eq!(
        block_list.block_heights.summary().height,
        expected_total_height_after_removal
    );

    // Remove the second banner
    block_list.remove_inline_banner(1);
    assert_eq!(
        block_list.block_heights.summary().block_count,
        total_block_count_after_insertion
    );
    assert_eq!(
        block_list.block_heights.summary().total_count,
        total_count_after_insertion - 2
    );

    let expected_total_height = expected_total_height - 2. * INLINE_BANNER_HEIGHT;
    assert_lines_approx_eq!(
        block_list.block_heights.summary().height,
        expected_total_height
    );
}

/// Regression test for WAR-6056, an issue where removing a banner would leave
/// the active gap in an incorrect state, causing a panic on the next window resize.
#[test]
fn test_gap_after_banner() {
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());

    // Create the following blocklist:
    // bootstrap block -> bootstrap block -> block -> banner -> gap -> block -> active block
    insert_block(&mut block_list, "cmd", "output");

    block_list.append_inline_banner(InlineBannerItem::new(
        0,
        InlineBannerType::NotificationsDiscovery,
    ));
    block_list.set_next_gap_height_in_lines(17.0.into_lines());
    block_list.clear_visible_screen();

    insert_block(&mut block_list, "cmd2", "output2");
    let baseline_block_height =
        block_list.blocks[2].height(&crate::terminal::model::block::TranscriptScope::Terminal);

    {
        let summary = block_list.block_heights.summary();
        let active_gap = block_list.active_gap.as_ref().unwrap().clone();
        let gap_height = 17. - baseline_block_height.as_f64();
        // There are 2 bootstrap blocks, 2 blocks, 1 banner, 1 gap, and 1 active block.
        assert_eq!(summary.total_count, 7);
        assert_lines_approx_eq!(active_gap.current_height, gap_height);
        // The gap is after the 2 bootstrap blocks, the first complete block, and the banner.
        assert_eq!(active_gap.index, 4);
        assert_lines_approx_eq!(
            summary.height,
            2. * baseline_block_height.as_f64() + INLINE_BANNER_HEIGHT + gap_height
        );
    }

    // Now, remove the banner and confirm that the gap was updated.
    block_list.remove_inline_banner(0);

    {
        let summary = block_list.block_heights.summary();
        let active_gap = block_list.active_gap.as_ref().unwrap().clone();
        let gap_height = 17. - baseline_block_height.as_f64();
        assert_eq!(summary.total_count, 6);
        assert_lines_approx_eq!(active_gap.current_height, gap_height);
        assert_eq!(active_gap.index, 3);
        assert_lines_approx_eq!(
            summary.height,
            2. * baseline_block_height.as_f64() + gap_height
        );
    }

    // Finally, resizing should update the gap without a panic.
    block_list.update_active_block_height();
    let size_update = SizeUpdate {
        update_reason: SizeUpdateReason::Refresh,
        last_size: *block_list.size(),
        new_size: SizeInfo::new_without_font_metrics(5, 5),
        new_gap_height: Some(5.0.into_lines()),
        natural_rows: 5,
        natural_cols: 5,
    };
    block_list.resize(&size_update, true);

    {
        let active_gap = block_list.active_gap.as_ref().unwrap().clone();
        let new_block_height =
            block_list.blocks[2].height(&crate::terminal::model::block::TranscriptScope::Terminal);
        assert_lines_approx_eq!(active_gap.current_height, 5.);
        assert_eq!(active_gap.index, 3);

        let mut cursor = block_list
            .block_heights
            .cursor::<TotalIndex, BlockHeightSummary>();
        assert!(cursor.seek(&active_gap.index(), SeekBias::Right));
        match cursor.item() {
            Some(BlockHeightItem::Gap(height)) => assert_lines_approx_eq!(height.0, 5.),
            other => panic!("Expected a Gap, got {other:?}"),
        }
        // The height only includes one block since the gap is before the second one.
        assert_lines_approx_eq!(cursor.end().height, new_block_height + 5.);

        assert_lines_approx_eq!(
            block_list.block_heights.summary().height,
            2. * new_block_height.as_f64() + 5.
        );
    }
}

#[test]
fn test_removed_gap_with_banner() {
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());

    block_list.set_next_gap_height_in_lines(17.0.into_lines());
    block_list.clear_visible_screen();
    assert!(block_list.active_gap.is_some());

    insert_block(&mut block_list, "cmd", "output");

    block_list.append_inline_banner(InlineBannerItem::new(
        0,
        InlineBannerType::NotificationsDiscovery,
    ));
    // Make sure the banner was inserted.
    assert!(
        block_list
            .block_heights
            .items()
            .iter()
            .any(|it| matches!(it, BlockHeightItem::InlineBanner { banner, .. } if banner.id == 0))
    );

    // There's two bootstrap blocks, one gap and one block before the banner.
    assert_eq!(
        block_list
            .removable_blocklist_item_positions
            .get(&RemovableBlocklistItem::InlineBanner(0)),
        Some(&TotalIndex(4))
    );

    // Add output so that the gap is removed by update_active_block_height.
    block_list.active_block_mut().finish(0);
    block_list.update_active_block_height();
    command_finished_and_precmd(&mut block_list);
    block_list.start_active_block();
    for _ in 0..20 {
        input_string(&mut block_list, "text");
        block_list.linefeed();
    }

    // Pretend that we finished processing a chunk of bytes from the PTY so we
    // properly update block heights.
    block_list.on_finish_byte_processing(&ansi::ProcessorInput::new(&[]));

    assert!(block_list.active_gap.is_none());
    assert_eq!(
        block_list
            .removable_blocklist_item_positions
            .get(&RemovableBlocklistItem::InlineBanner(0)),
        Some(&TotalIndex(3))
    );

    // We should still be able to remove the banner even though its position has changed.
    block_list.remove_inline_banner(0);
    assert_eq!(
        block_list
            .block_heights
            .items()
            .iter()
            .find(|it| matches!(it, BlockHeightItem::InlineBanner { .. })),
        None
    );
}

#[test]
pub fn test_block_heights_combined_prompt_command_grid_warp_prompt() {
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());
    let bootstrapped_block_list_len = block_list.blocks().len();

    // Create one block with 3 command lines and 3 output lines.
    let first_block_index = insert_block(&mut block_list, "foo\nbar\nbazz\n", "foo\nbar\nbazz\n");

    let first_block = block_list
        .block_at(first_block_index)
        .expect("block should exist");

    // We created one block.
    assert_eq!(block_list.blocks.len(), bootstrapped_block_list_len + 1);

    // Note that this test is using the Warp prompt, hence the prompt is not included in the combined grid.
    assert_eq!(first_block.prompt_and_command_grid().len(), 3);
    assert_eq!(first_block.output_grid().len(), 3);

    // In this case, we SHOULD consider command_padding_top since we have a combined prompt/command grid BUT
    // we have the built-in Warp prompt, so there's padding between that prompt and the combined grid.
    // The combined grid _just_ has the command in this case! The PS1 is unset!
    // Hence, we expect heights of 8.5.
    assert_lines_approx_eq!(
        first_block.height(&crate::terminal::model::block::TranscriptScope::Terminal),
        8.5
    );
}

#[test]
pub fn test_block_heights_combined_prompt_command_grid_ps1() {
    let block_sizes = BlockSize {
        // Make sure the grid is wide enough that "prompt2" + "foo" fits on one
        // line without wrapping.
        size: SizeInfo::new_without_font_metrics(10, 20),
        ..test_utils::block_size()
    };
    let mut block_list = new_bootstrapped_block_list(
        Some(block_sizes),
        Some(true),
        ChannelEventListener::new_for_test(),
    );

    let bootstrapped_block_list_len = block_list.blocks().len();

    // Create one block with 3 prompt/command lines (1 prompt line, 1 combined line, 2 command lines) and 3 output lines.
    let first_block_index = insert_block_with_prompt(
        &mut block_list,
        "prompt1\nprompt2",
        "foo\nbar\nbazz\n",
        "foo\nbar\nbazz\n",
    );

    let first_block = block_list
        .block_at(first_block_index)
        .expect("block should exist");

    // We created one block.
    assert_eq!(block_list.blocks.len(), bootstrapped_block_list_len + 1);

    // We have a 2-line prompt, but the second line should be shared with the command!
    // Hence the 2-line prompt and 3-line command result in 4 total lines!
    assert_eq!(first_block.prompt_and_command_grid().len(), 4);
    assert_eq!(first_block.output_grid().len(), 3);

    // We have a 2-line prompt, adding 1 extra line to the combined grid (vs 0.6 default for Warp prompt).
    // Hence, we expect a height of 8.7 rather than 8.3.
    assert_lines_approx_eq!(
        first_block.height(&crate::terminal::model::block::TranscriptScope::Terminal),
        8.7
    );
}

#[test]
fn test_block_height_update_shifts_indices() {
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());

    // Create a dummy block.
    let first_block_index = insert_block(&mut block_list, "cmd", "output");
    assert_eq!(first_block_index, BlockIndex(2));
    assert_eq!(first_block_index.to_total_index(&block_list), TotalIndex(2));

    // Insert a gap after the created block. It should be at total index 3 (after the first 2 bootstrap blocks + inserted block).
    block_list.set_next_gap_height_in_lines(17.0.into_lines());
    block_list.clear_visible_screen();
    assert!(block_list.active_gap.is_some());
    assert_eq!(
        block_list.active_gap.as_ref().unwrap().index(),
        TotalIndex(3)
    );

    // Create another dummy block.
    let second_block_index = insert_block(&mut block_list, "cmd", "output");
    assert_eq!(second_block_index, BlockIndex(3));
    assert_eq!(
        second_block_index.to_total_index(&block_list),
        TotalIndex(4)
    );

    // Insert a banner before the first block.
    block_list.insert_inline_banner_before_block(
        first_block_index,
        InlineBannerItem::new(0, InlineBannerType::NotificationsDiscovery),
        None,
    );

    // Make sure the banner was inserted.
    assert!(
        block_list
            .block_heights
            .items()
            .iter()
            .any(|it| matches!(it, BlockHeightItem::InlineBanner { banner, .. } if banner.id == 0))
    );
    assert_eq!(
        block_list
            .removable_blocklist_item_positions
            .get(&RemovableBlocklistItem::InlineBanner(0)),
        Some(&TotalIndex(2))
    );

    // Make sure the gap was adjusted.
    assert!(block_list.active_gap.is_some());
    assert_eq!(
        block_list.active_gap.as_ref().unwrap().index(),
        TotalIndex(4)
    );

    // Remove the banner
    block_list.remove_inline_banner(0);

    // Make sure the banner is gone.
    assert!(
        !block_list
            .block_heights
            .items()
            .iter()
            .any(|it| matches!(&it, BlockHeightItem::InlineBanner { .. }))
    );
    assert!(
        !block_list
            .removable_blocklist_item_positions
            .contains_key(&RemovableBlocklistItem::InlineBanner(0))
    );

    // Make sure the gap was adjusted back.
    assert!(block_list.active_gap.is_some());
    assert_eq!(
        block_list.active_gap.as_ref().unwrap().index(),
        TotalIndex(3)
    );
}








#[test]
pub fn clear_blocks_resets_index() {
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());

    // The above call adds 3 blocks to the block list.
    assert_eq!(block_list.blocks.len(), 3);

    // Create 4 extra blocks -- the block list should have a total size of 7.
    for _ in 0..4 {
        insert_block(&mut block_list, "foo\nbar\nbazz", "foo\nbar\nbazz");
    }

    assert_eq!(block_list.blocks.len(), 7);
    assert_eq!(block_list.active_block().index(), 6.into());

    // Clear the screen and ensure the block index is reset properly.
    block_list.clear_screen(ClearMode::ResetAndClear);
    assert_eq!(block_list.active_block().index(), 0.into());
}






#[test]
fn test_interleaves_background_with_gaps() {
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());
    block_list.set_next_gap_height_in_lines(17.0.into_lines());

    insert_block(&mut block_list, "some background command &\n", "\n");
    input_string(&mut block_list, "bg1");
    block_list.carriage_return();
    block_list.linefeed();
    block_list.clear_visible_screen();
    assert_lines_approx_eq!(block_list.active_gap().unwrap().height(), 17.0);
    input_string(&mut block_list, "bg2");
    block_list.carriage_return();
    block_list.linefeed();
    block_list.on_finish_byte_processing(&ansi::ProcessorInput::new(&[]));

    // There are 2 bootstrap blocks, 1 command block, 2 background blocks, and 1 active block.
    assert_eq!(block_list.blocks.len(), 6);

    assert_eq!(
        &block_list.blocks[2].command_to_string(),
        "some background command &"
    );
    assert_eq!(&block_list.blocks[3].output_to_string(), "bg1");
    assert!(block_list.blocks[3].is_background());
    assert!(block_list.blocks[3].finished());
    // The second background block isn't finished, so the trailing \n has not
    // been stripped off yet.
    assert_eq!(&block_list.blocks[4].output_to_string(), "bg2\n");
    assert!(block_list.blocks[4].is_background());
    assert!(!block_list.blocks[4].finished());

    let expected_heights = [
        BlockHeightItem::Block(0.0.into()),
        BlockHeightItem::Block(0.0.into()),
        BlockHeightItem::Block(7.5.into()),
        // The first background block should be before the gap.
        BlockHeightItem::Block(2.2.into()),
        // The second background block should have shrunk the gap.
        BlockHeightItem::Gap(14.6.into()),
        BlockHeightItem::Block(2.4.into()),
        // The active block has 0 height.
        BlockHeightItem::Block(0.0.into()),
    ];
    assert_eq!(
        block_list.block_heights.items().len(),
        expected_heights.len()
    );
    for (actual, expected) in block_list
        .block_heights
        .items()
        .iter()
        .zip(expected_heights.iter())
    {
        // Make sure the items are of the same type.
        assert_eq!(
            std::mem::discriminant(actual),
            std::mem::discriminant(expected)
        );
        // Make sure the heights are the same.
        assert_approx_eq!(
            Lines,
            actual.height().into_lines(),
            expected.height().into_lines()
        );
    }
}

#[test]
fn test_remove_background_block_with_active_gap() {
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());

    block_list.set_next_gap_height_in_lines(17.0.into_lines());
    block_list.clear_visible_screen();

    input_string(&mut block_list, "echo foo");

    assert!(block_list.active_gap().is_some());

    // This is the real part of the test: if the fix is not working properly,
    // this will panic on a debug_assert
    block_list.remove_background_block();

    assert!(block_list.active_gap().is_some());
}

#[test]
fn test_device_status_uses_active_block_if_no_typeahead() {
    let mut block_list =
        new_bootstrapped_block_list(None, None, ChannelEventListener::new_for_test());

    insert_block(&mut block_list, "command\n", "output\n");
    let active_block = block_list.active_block_mut();
    let grid = active_block
        .grid_of_type_mut(active_block.active_grid_type())
        .expect("should have grid");
    grid.grid_handler_mut().update_cursor(|cursor| {
        cursor.point.col = 20;
    });
    assert_eq!(
        block_list.active_block().grid_handler().cursor_point(),
        Point { row: 0, col: 20 }
    );

    let mut writer = Vec::new();

    block_list.device_status(&mut writer, 6);

    assert_eq!(writer, "\x1b[1;21R".as_bytes());
}

