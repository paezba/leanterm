use std::sync::Arc;

use parking_lot::FairMutex;
use warpui::App;

use super::Body;
use crate::terminal::TerminalModel;
use crate::terminal::shared_session::{
    MAX_BYTES_SHAREABLE, SharedSessionActionSource, SharedSessionScrollbackType,
};
use crate::test_util::terminal::initialize_app_for_terminal_view;





