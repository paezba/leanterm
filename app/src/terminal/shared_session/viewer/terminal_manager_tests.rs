//! Regression tests for the viewer `TerminalManager`'s `on_view_detached`
//! discriminator and the OVM-teardown helper.
//!
//! Before the fix, closing a viewer pane (tab close / split-pane close) did
//! not flow through any of the network-event paths
//! (`SessionEnded` / `ViewerRemoved` / `FailedToReconnect`), so the
//! orchestration viewer model — and its viewer-mode registration on the
//! shared [`OrchestrationEventStreamer`] — leaked until the app exited.
//! `TerminalManager::on_view_detached` now tears down the OVM on
//! `DetachType::Closed`, while deliberately preserving it on
//! `HiddenForClose` (undo-close grace window) and `Moved`.


// Bring the `TerminalManager` trait into scope (named under a different alias
// since the local `TerminalManager` struct shadows it) so the trait method
// `on_view_detached` is callable on the struct.

/// Stub UUID used for the orchestrator's `AmbientAgentTaskId`; opaque to
/// the manager.
const PARENT_TASK_ID: &str = "11111111-1111-1111-1111-111111111111";







