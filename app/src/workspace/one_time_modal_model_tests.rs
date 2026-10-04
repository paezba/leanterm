use std::time::SystemTime;

use ai::api_keys::{ApiKeyManager, ChatGPTConnection, ChatGPTConnectionStatus};
use futures::FutureExt;
use settings::Setting as _;
use warp_core::features::FeatureFlag;
use warpui::{App, SingletonEntity, WindowId};

use super::{ AuthManager, AuthManagerEvent, CloudPreferencesSyncer, OneTimeModalModel, hoa_onboarding,
};
use crate::test_util::terminal::{ initialize_app_for_terminal_view,
};
use crate::workspaces::workspace::CustomerType;

/// Registers the cloud preferences syncer on top of the standard terminal test setup, and
/// returns the window the ChatGPT plan modal would target.

fn set_chatgpt_connected(token_sharing_active: bool, app: &mut App) {
    ApiKeyManager::handle(app).update(app, |manager, ctx| {
        manager.set_chatgpt_connection_status(
            ChatGPTConnectionStatus::Connected(ChatGPTConnection {
                email: None,
                connected_at: SystemTime::now(),
                token_sharing_active,
            }),
            ctx,
        );
    });
}

fn mark_cloud_preferences_loaded(app: &mut App) {
    CloudPreferencesSyncer::handle(app).update(app, |syncer, _| {
        syncer.mark_initial_load_completed_for_test();
    });
}



















