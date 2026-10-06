use std::sync::Arc;

use leanterm_core::context_flag::ContextFlag;
use leanterm_ui::{Entity, ModelContext, SingletonEntity};

use crate::server::network_logging::NetworkLogModel;

/// Singleton that owns the shared HTTP client used for outbound requests.
pub struct HttpClientProvider {
    client: Arc<http_client::Client>,
}

impl HttpClientProvider {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        let mut client = http_client::Client::new();
        if ContextFlag::NetworkLogConsole.is_enabled() {
            NetworkLogModel::handle(ctx).update(ctx, |model, model_ctx| {
                model.install_on_clients([&mut client], model_ctx);
            });
        }
        Self {
            client: Arc::new(client),
        }
    }

    /// Returns the shared HTTP client, which is wired into network logging.
    pub fn client(&self) -> Arc<http_client::Client> {
        self.client.clone()
    }
}

impl Entity for HttpClientProvider {
    type Event = ();
}

impl SingletonEntity for HttpClientProvider {}
