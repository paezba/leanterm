use std::time::SystemTime;

use warpui::{App, SingletonEntity};

use super::CloudPreferencesSyncer;

/// Registers the cloud preferences syncer on top of the standard terminal test setup, and
/// returns the window the ChatGPT plan modal would target.


fn mark_cloud_preferences_loaded(app: &mut App) {
    CloudPreferencesSyncer::handle(app).update(app, |syncer, _| {
        syncer.mark_initial_load_completed_for_test();
    });
}



















