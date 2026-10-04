use tracing::subscriber;

/// Installs a no-op global `tracing` subscriber.
///
/// This prevents the `tracing` crate from writing out log lines for spans and trace events.
pub fn init() -> anyhow::Result<Initialization> {
    subscriber::set_global_default(subscriber::NoSubscriber::new())?;
    Ok(Initialization)
}

/// The result of [`init`]; kept so callers have a single handle to the tracing lifecycle.
#[derive(Default)]
pub struct Initialization;

impl Initialization {
    pub fn log_initialization_warning(&mut self) {}

    pub(crate) fn shutdown(&mut self) {}
}
