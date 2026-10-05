use std::sync::Arc;

use warpui::r#async::executor::Background;

use crate::auth::auth_state::AuthState;

/// Determine the Warp download method (if possible) and send a telemetry event reporting that
/// method
pub fn determine_and_report(_auth_state: Arc<AuthState>, executor: Arc<Background>) {
    executor
        .spawn(async move {

        })
        .detach();
}

#[cfg(not(target_os = "macos"))]
async fn check_download_source() -> DownloadSource {
    DownloadSource::Website
}
