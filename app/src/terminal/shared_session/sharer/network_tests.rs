use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_channel::Sender;
use byte_unit::Byte;
use futures::channel::mpsc;
use futures_util::future::BoxFuture;
use futures_util::stream::AbortHandle;
use futures_util::{FutureExt as _, SinkExt as _, StreamExt as _, future, sink, stream};
use instant::Instant;
use parking_lot::FairMutex;
use session_sharing_protocol::common::{
    ActivePrompt, FeatureSupport, InputOperationId, InputOperationSeqNo, InputUpdate,
    OrderedTerminalEvent, OrderedTerminalEventType, ParticipantId, Selection, SelectionUpdate,
    SessionId, UserID,
};
use session_sharing_protocol::sharer::{
    DownstreamMessage, FailedToInitializeSessionReason, QuotaType, ReconnectPayload,
    ReconnectToken, ReconnectionFailedReason, SessionEndedReason, SessionTerminatedReason,
    UpstreamMessage,
};
use warp_server_client::iap::IapManager;
#[cfg(not(target_family = "wasm"))]
use warpui::r#async::executor::Foreground;
use warpui::r#async::{FutureExt as _, Timer};
use warpui::{App, ModelHandle, RetryOption};
use websocket::{Error as WebsocketError, Message, Sink, Stream, WebsocketMessage as _};

use super::{
    AMBIENT_CREATE_SESSION_MAX_ATTEMPTS, ConfirmedReconnection, MAX_PRE_RECONNECT_BYTES,
    MAX_PRE_RECONNECT_MESSAGES, Network, NetworkEvent, PTY_READS_BATCH_THRESHOLD,
    PtyBytesBatchStatus, Stage, StartupFailure, StartupRetryState, confirm_reconnection,
    share_with_team_uid_for_init_payload, startup_max_attempts,
};
use crate::auth::AuthStateProvider;
use crate::auth::auth_manager::AuthManager;
use crate::server::server_api::ServerApiProvider;
use crate::server::telemetry::context_provider::AppTelemetryContextProvider;
use crate::terminal::TerminalModel;
use crate::terminal::shared_session::{
    MAX_BYTES_SHAREABLE, SELECTION_THROTTLE_PERIOD, SharedSessionSource,
};
use crate::test_util::assert_eventually;

fn is_upstream_message_pty_bytes_read(
    message: UpstreamMessage,
    expected_event_no: usize,
    expected_bytes: Vec<u8>,
) -> bool {
    let compressed_bytes = lz4_flex::block::compress_prepend_size(&expected_bytes);
    matches!(message, UpstreamMessage::OrderedTerminalEvent(OrderedTerminalEvent {
        event_no,
        event_type: OrderedTerminalEventType::PtyBytesRead { bytes },
    }) if event_no == expected_event_no && bytes == compressed_bytes)
}

fn discard_sink() -> impl Sink {
    sink::drain().sink_map_err(|error: Infallible| match error {})
}

fn reconnect_payload() -> ReconnectPayload {
    ReconnectPayload {
        session_secret: Default::default(),
        reconnect_token: ReconnectToken::new(),
        user_id: UserID {
            anonymous_id: "anonymous".to_string(),
            access_token: None,
        },
        latest_block_id: "block".to_string().into(),
        selection: Selection::None,
        feature_support: FeatureSupport {
            supports_agent_view: false,
            supports_full_role: true,
            supports_full_role_for_real: true,
        },
    }
}

fn reconnected_message() -> Message {
    Message::new(
        DownstreamMessage::SessionReconnected {
            last_received_event_no: None,
            participant_list: Default::default(),
        }
        .to_json()
        .unwrap(),
    )
}

type MockReconnection = ConfirmedReconnection<Pin<Box<dyn Sink>>, Pin<Box<dyn Stream>>>;
type ReconnectAttempt = BoxFuture<'static, anyhow::Result<MockReconnection>>;

fn mock_reconnect(stream: impl Stream) -> ReconnectAttempt {
    mock_reconnect_with_sink(discard_sink(), stream)
}

fn mock_reconnect_with_sink(sink: impl Sink, stream: impl Stream) -> ReconnectAttempt {
    let sink: Pin<Box<dyn Sink>> = Box::pin(sink);
    let stream: Pin<Box<dyn Stream>> = Box::pin(stream);
    confirm_reconnection(sink, stream, reconnect_payload()).boxed()
}

fn confirmed_reconnect() -> ReconnectAttempt {
    mock_reconnect(stream::iter([Ok(reconnected_message())]).chain(stream::pending()))
}












#[test]
fn test_reconnect_buffers_pre_ack_messages_and_preserves_remaining_stream() {
    App::test((), |_| async move {
        let buffered = DownstreamMessage::EventsProcessedAck {
            latest_processed_event_no: 1,
        };
        let connection = mock_reconnect(stream::iter([
            Ok(Message::new_binary(vec![1])),
            Ok(Message::new(buffered.to_json().unwrap())),
            Ok(reconnected_message()),
            Ok(Message::new(buffered.to_json().unwrap())),
        ]))
        .await
        .unwrap();
        assert_eq!(connection.buffered_messages.len(), 1);
        assert_eq!(
            connection.buffered_messages[0].text(),
            Some(buffered.to_json().unwrap().as_str())
        );
        let mut remaining = connection.stream;
        assert!(remaining.next().await.unwrap().is_ok());
    });
}

#[test]
fn test_reconnect_pre_ack_buffer_rejects_message_count_over_limit() {
    App::test((), |_| async move {
        let messages = (0..=MAX_PRE_RECONNECT_MESSAGES).map(|_| Ok(Message::new("{}".to_string())));
        let error = mock_reconnect(stream::iter(messages)).await.err().unwrap();
        assert!(error.to_string().contains("Too many messages"));
    });
}

#[test]
fn test_reconnect_pre_ack_buffer_accepts_message_count_at_limit() {
    App::test((), |_| async move {
        let messages = (0..MAX_PRE_RECONNECT_MESSAGES).map(|_| Ok(Message::new("{}".to_string())));
        let connection =
            mock_reconnect(stream::iter(messages).chain(stream::iter([Ok(reconnected_message())])))
                .await
                .unwrap();
        assert_eq!(
            connection.buffered_messages.len(),
            MAX_PRE_RECONNECT_MESSAGES
        );
    });
}

#[test]
fn test_reconnect_pre_ack_buffer_rejects_byte_count_over_limit() {
    App::test((), |_| async move {
        let error = mock_reconnect(stream::iter([Ok(Message::new(
            "x".repeat(MAX_PRE_RECONNECT_BYTES + 1),
        ))]))
        .await
        .err()
        .unwrap();
        assert!(error.to_string().contains("Too many messages"));
    });
}

#[test]
fn test_reconnect_pre_ack_buffer_accepts_byte_count_at_limit() {
    App::test((), |_| async move {
        let connection = mock_reconnect(stream::iter([
            Ok(Message::new("x".repeat(MAX_PRE_RECONNECT_BYTES))),
            Ok(reconnected_message()),
        ]))
        .await
        .unwrap();
        assert_eq!(connection.buffered_messages.len(), 1);
        assert_eq!(
            connection.buffered_messages[0].text().unwrap().len(),
            MAX_PRE_RECONNECT_BYTES
        );
    });
}




#[cfg(not(target_family = "wasm"))]
async fn finish_foreground_tasks(app: &App) {
    let foreground = app.foreground_executor();
    let Foreground::Test { executor } = foreground.as_ref() else {
        panic!("Expected the test foreground executor");
    };
    // The foreground stream task remains registered until both on_item and on_done return.
    // Closing the unrelated test sources lets executor emptiness prove callback completion.
    assert_eventually!(400 => executor.is_empty(), "Old websocket callbacks should finish");
}






#[test]
fn test_startup_max_attempts_only_retries_ambient_agent_sources() {
    assert_eq!(
        startup_max_attempts(&SharedSessionSource::ambient_agent(Some(
            "task-id".to_string()
        ))),
        AMBIENT_CREATE_SESSION_MAX_ATTEMPTS
    );
    assert_eq!(startup_max_attempts(&SharedSessionSource::user(None)), 1);
}

#[test]
fn test_startup_failure_retryability() {
    assert!(StartupFailure::Transport.is_retryable());
    assert!(StartupFailure::InitializeSend.is_retryable());
    assert!(StartupFailure::WebsocketClosedBeforeStarted.is_retryable());
    assert!(StartupFailure::WebsocketError.is_retryable());
    assert!(StartupFailure::Timeout.is_retryable());
    assert!(
        StartupFailure::ServerRejected(FailedToInitializeSessionReason::InternalServerError {
            details: "transient".to_string(),
        })
        .is_retryable()
    );

    assert!(
        !StartupFailure::ServerRejected(FailedToInitializeSessionReason::ScrollbackTooLarge {})
            .is_retryable()
    );
    assert!(
        !StartupFailure::ServerRejected(FailedToInitializeSessionReason::NoUserQuotaRemaining {
            quota_type: QuotaType::SessionsCreated,
        })
        .is_retryable()
    );
    assert!(
        !StartupFailure::ServerRejected(FailedToInitializeSessionReason::UserNotFound)
            .is_retryable()
    );
}



fn is_upstream_message_command_executed(
    message: &UpstreamMessage,
    expected_event_no: usize,
) -> bool {
    matches!(message, UpstreamMessage::OrderedTerminalEvent(OrderedTerminalEvent {
        event_no,
        event_type: OrderedTerminalEventType::CommandExecutionStarted { .. },
    }) if *event_no == expected_event_no)
}

fn is_upstream_message_selection_update(
    message: UpstreamMessage,
    expected_event_no: usize,
    expected_selection: Selection,
) -> bool {
    matches!(
        message,
        UpstreamMessage::UpdateSelection(SelectionUpdate {
            selection,
            event_no,
        }) if event_no == expected_event_no.into() && selection == expected_selection
    )
}








/// Waits until the mock terminal model reports its active block as bootstrapped.
///
/// `start_ordered_terminal_events_listener` silently drops ordered events until this is
/// true, so callers must wait for it instead of racing it: sending an event beforehand can
/// flake if the listener task hasn't observed the bootstrapped state yet. Uses the same
/// generous 2s budget as the `recv()` timeouts below it, rather than the default
/// `assert_eventually!` tick budget, so this wait can't reintroduce a fixed-window race of
/// its own.
async fn wait_for_bootstrapped(network: &ModelHandle<Network>, app: &App) {
    assert_eventually!(
        400 =>
        network.read(app, |network, _ctx| network
            .model
            .lock()
            .is_active_block_bootstrapped()),
        "Mock terminal model should report the active block as bootstrapped"
    );
}







