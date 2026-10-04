
use session_sharing_protocol::common::{
    OrderedTerminalEvent, OrderedTerminalEventType, Scrollback, ScrollbackBlock,
};
use warpui::platform::WindowStyle;
use warpui::{App, ViewHandle};

use crate::terminal::TerminalView;
use crate::terminal::model::block::{BlockId, SerializedBlock};
use crate::test_util::terminal::initialize_app_for_terminal_view;
