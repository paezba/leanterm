use std::sync::Arc;
use std::time::Duration;

use async_channel::Sender;
use async_io::Timer;
use instant::Instant;
use parking_lot::FairMutex;
use session_sharing_protocol::viewer::UpstreamMessage;
use warpui::{App, ModelHandle};

use super::{Network, PtyBytesBatchStatus, Stage};
use crate::terminal::TerminalModel;
use crate::terminal::event_listener::ChannelEventListener;
use crate::terminal::shared_session::shared_handlers::RemoteUpdateGuard;
use crate::test_util::terminal::initialize_app_for_terminal_view;




