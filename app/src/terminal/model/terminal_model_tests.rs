use chrono::Local;
use warp_core::command::ExitCode;

use super::*;
use crate::terminal::model::block::BlockId;
use crate::terminal::model::bootstrap::BootstrapStage;

// Ensures that an SSH session successfully bootstraps even if the block list is empty and that
// the parent shell resumes after the nested shell exits.

#[test]
fn test_selected_block_range_contains() {
    let range = SelectedBlockRange {
        pivot: 10.into(),
        tail: 2.into(),
    };
    assert!(range.contains(4.into()));
    assert!(!range.contains(1.into()));
}

#[test]
fn test_selected_blocks_is_selected() {
    let selected_blocks = SelectedBlocks {
        ranges: vec![
            SelectedBlockRange {
                pivot: 3.into(),
                tail: 4.into(),
            },
            SelectedBlockRange {
                pivot: 0.into(),
                tail: 1.into(),
            },
        ],
    };

    assert!(selected_blocks.is_selected(0.into()));
    assert!(!selected_blocks.is_selected(2.into()));
}

#[test]
fn test_selected_blocks_tail() {
    let selected_blocks = SelectedBlocks {
        ranges: vec![
            SelectedBlockRange {
                pivot: 3.into(),
                tail: 4.into(),
            },
            SelectedBlockRange {
                pivot: 0.into(),
                tail: 1.into(),
            },
        ],
    };

    assert_eq!(selected_blocks.tail(), Some(1.into()));
}

#[test]
fn test_selected_blocks_reset() {
    let mut selected_blocks = SelectedBlocks {
        ranges: vec![
            SelectedBlockRange {
                pivot: 3.into(),
                tail: 4.into(),
            },
            SelectedBlockRange {
                pivot: 0.into(),
                tail: 1.into(),
            },
        ],
    };

    // Reset to nothing.
    selected_blocks.reset();
    assert!(selected_blocks.is_empty());
    assert!(selected_blocks.tail().is_none());

    // Reset to single.
    selected_blocks.reset_to_single(5.into());
    assert!(!selected_blocks.is_empty());
    assert_eq!(selected_blocks.tail(), Some(5.into()));
}

#[test]
fn test_selected_blocks_range_select() {
    let mut selected_blocks = SelectedBlocks {
        ranges: vec![
            SelectedBlockRange {
                pivot: 0.into(),
                tail: 1.into(),
            },
            SelectedBlockRange {
                pivot: 3.into(),
                tail: 5.into(),
            },
        ],
    };

    // Range select should reset to a single selection with
    // same pivot as most recent selection, and new tail.
    selected_blocks.range_select(6.into());
    assert_eq!(selected_blocks.ranges().len(), 1);
    assert_eq!(selected_blocks.ranges().last().unwrap().pivot, 3.into());
    assert_eq!(selected_blocks.tail(), Some(6.into()));

    // Range select reversed should work similarly.
    selected_blocks.ranges = vec![
        SelectedBlockRange {
            pivot: 0.into(),
            tail: 1.into(),
        },
        SelectedBlockRange {
            pivot: 3.into(),
            tail: 5.into(),
        },
    ];
    selected_blocks.range_select(0.into());
    assert_eq!(selected_blocks.ranges().len(), 1);
    assert_eq!(selected_blocks.ranges().last().unwrap().pivot, 3.into());
    assert_eq!(selected_blocks.tail(), Some(0.into()));
}

#[test]
fn compare_within_block_points() {
    let a = WithinBlock::new(Point::new(4, 5), 1.into(), GridType::PromptAndCommand);
    let b = WithinBlock::new(Point::new(1, 0), 2.into(), GridType::PromptAndCommand);
    assert!(a < b);

    let c = WithinBlock::new(Point::new(1, 5), 2.into(), GridType::Output);
    let d = WithinBlock::new(Point::new(4, 0), 2.into(), GridType::PromptAndCommand);
    assert!(d < c);

    let e = WithinBlock::new(Point::new(1, 5), 2.into(), GridType::PromptAndCommand);
    let f = WithinBlock::new(Point::new(4, 0), 2.into(), GridType::PromptAndCommand);
    assert!(e < f);

    let g = WithinBlock::new(Point::new(1, 5), 2.into(), GridType::PromptAndCommand);
    let h = WithinBlock::new(Point::new(1, 4), 2.into(), GridType::PromptAndCommand);
    assert!(h < g);

    let i = WithinBlock::new(Point::new(1, 5), 2.into(), GridType::PromptAndCommand);
    let j = WithinBlock::new(Point::new(1, 5), 2.into(), GridType::PromptAndCommand);
    assert!(i == j);
}

#[test]
// An empty block that is restored should have a nonzero height and it should not get deleted.
pub fn test_restored_empty_command_block() {
    let restored_blocks = [create_default_serialized_block().into()];
    let model = TerminalModel::mock(Some(&restored_blocks), None);
    let restored_block = &model.block_list().blocks()[0];
    assert_eq!(
        restored_block.bootstrap_stage(),
        BootstrapStage::RestoreBlocks
    );
    assert!(
        !restored_block.is_command_empty(),
        "The empty block should have nonzero length"
    );
    // The mocked terminal model comes with a WarpInput block and the active block.
    assert_eq!(model.block_list().blocks().len(), 3);
}

/// Helper function to create a SerializedBlock with default values,
/// including the new is_local field.
fn create_default_serialized_block() -> SerializedBlock {
    SerializedBlock {
        id: BlockId::new(),
        stylized_command: Default::default(),
        stylized_output: Default::default(),
        pwd: None,
        git_head: None,
        git_branch_name: None,
        virtual_env: None,
        conda_env: None,
        node_version: None,
        exit_code: ExitCode::from(0),
        did_execute: false,
        start_ts: Some(Local::now()),
        completed_ts: Some(Local::now()),
        ps1: None,
        rprompt: None,
        honor_ps1: false,
        session_id: None,
        shell_host: None,
        is_background: false,
        prompt_snapshot: None,
        is_local: None,
    }
}
