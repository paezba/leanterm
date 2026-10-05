use std::sync::Arc;

use warp_core::features::FeatureFlag;
use warp_core::settings::Setting;
use warp_errors::report_if_error;
use warpui::{Entity, ModelContext, SingletonEntity};

use crate::settings::{FontSettings, PrivacySettings, ThemeSettings};
use crate::themes::theme::ThemeKind;

pub struct SettingsInitializer;

impl Default for SettingsInitializer {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsInitializer {
    pub fn new() -> Self {
        Self
    }

}

impl Entity for SettingsInitializer {
    type Event = ();
}

/// Mark CloudPreferencesSyncer as global application state.
impl SingletonEntity for SettingsInitializer {}
