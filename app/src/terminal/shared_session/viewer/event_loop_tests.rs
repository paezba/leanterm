use std::sync::Arc;

use parking_lot::FairMutex;
use session_sharing_protocol::common::{
    OrderedTerminalEvent, OrderedTerminalEventType, Scrollback, ScrollbackBlock, WindowSize,
};
use warp_core::command::ExitCode;
use warp_core::features::FeatureFlag;
use warpui::platform::WindowStyle;
use warpui::units::Lines;
use warpui::{App, SingletonEntity, ViewHandle};

use crate::terminal::TerminalView;
use crate::terminal::event_listener::ChannelEventListener;
use crate::terminal::model::block::{BlockId, BlockState, SerializedBlock};
use crate::terminal::shared_session::SharedSessionStatus;
use crate::terminal::shared_session::shared_handlers::RemoteUpdateGuard;
use crate::terminal::shared_session::tests::terminal_model_for_viewer;
use crate::terminal::shared_session::viewer::event_loop::{
    EventLoop, SharedSessionInitialLoadMode,
};
use crate::test_util::terminal::initialize_app_for_terminal_view;

fn ordered_terminal_event_from_bytes(
    bytes: impl Into<Vec<u8>>,
    event_no: usize,
) -> OrderedTerminalEvent {
    let compressed = lz4_flex::block::compress_prepend_size(&bytes.into());
    OrderedTerminalEvent {
        event_no,
        event_type: OrderedTerminalEventType::PtyBytesRead { bytes: compressed },
    }
}

fn old_sharer_dcs_bytes(payload: &str) -> Vec<u8> {
    let mut bytes = b"\x1bP$d".to_vec();
    bytes.extend(hex::encode(payload).bytes());
    bytes.push(0x9c);
    bytes
}


/// Cloud-mode terminal view counterpart to [`terminal_view`]. Sets up the
/// singletons and constructs a `TerminalView` with `is_cloud_mode = true` so
/// `ambient_agent_view_model()` is `Some(..)`.
fn cloud_mode_terminal_view(app: &mut App) -> ViewHandle<TerminalView> {
    initialize_app_for_terminal_view(app);
    let tips_model = app.add_model(|_| Default::default());
    let (_, terminal) = app.add_window(WindowStyle::NotStealFocus, |ctx| {
        TerminalView::new_for_test_with_cloud_mode(tips_model, None, true, ctx)
    });
    terminal
}

fn completed_block(command: &str, output: &str) -> SerializedBlock {
    let mut block =
        SerializedBlock::new_for_test(command.as_bytes().into(), output.as_bytes().into());
    block.id = BlockId::new();
    block
}

fn active_block() -> SerializedBlock {
    let mut block = SerializedBlock::new_active_block_for_test();
    block.id = BlockId::new();
    block
}

fn scrollback_block(block: &SerializedBlock) -> ScrollbackBlock {
    ScrollbackBlock {
        raw: serde_json::to_vec(block).unwrap(),
    }
}

fn empty_scrollback() -> Scrollback {
    Scrollback {
        blocks: vec![],
        is_alt_screen_active: false,
    }
}













