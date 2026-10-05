use settings::Setting as _;
use warpui::{Entity, ModelContext, SingletonEntity, WindowId};

use super::hoa_onboarding;
use crate::auth::AuthManager;
use crate::auth::auth_manager::AuthManagerEvent;
use crate::settings::CodeSettings;
use crate::settings::cloud_preferences_syncer::{
    CloudPreferencesSyncer, CloudPreferencesSyncerEvent,
};
use crate::terminal::general_settings::GeneralSettings;

/// A generic model for managing one-time modals that should be shown to users only once.
///
/// The model holds the canonical state of whether a modal is currently being shown and
/// automatically triggers the modal when appropriate conditions are met (e.g., user becomes
/// onboarded).
pub struct OneTimeModalModel {
    /// Whether the OpenWarp launch modal is currently being shown.
    is_openwarp_launch_modal_open: bool,
    /// Whether the HOA onboarding flow is currently being shown.
    is_hoa_onboarding_open: bool,
    /// The window ID where the currently open one-time modal should be displayed.
    /// This is captured when a modal is first opened and ensures the modal stays on that window.
    target_window_id: Option<WindowId>,
}

impl OneTimeModalModel {
    pub fn new(ctx: &mut ModelContext<Self>) -> Self {
        // Subscribe to auth manager events to automatically trigger modal when user becomes onboarded
        ctx.subscribe_to_model(&AuthManager::handle(ctx), |_, _, event, ctx| {
            let AuthManagerEvent::AuthComplete = event else {
                return;
            };

            let auth_state = crate::auth::AuthStateProvider::as_ref(ctx).get().clone();
            let is_existing_user = auth_state.is_onboarded().unwrap_or_default();
            if is_existing_user {
                // Settings modals settings are synced to the cloud, not respecting the user's sync setting, so they
                // must all await initial load to be triggered, else we risk reading a stale triggered value.
                ctx.subscribe_to_model(
                    &CloudPreferencesSyncer::handle(ctx),
                    move |me, _, event, ctx| {
                        if let CloudPreferencesSyncerEvent::InitialLoadCompleted = event {
                            ctx.unsubscribe_from_model(&CloudPreferencesSyncer::handle(ctx));
                            me.check_and_trigger_all_modals(ctx);
                        }
                    },
                );
            } else {
                hoa_onboarding::mark_hoa_onboarding_completed(ctx);
                GeneralSettings::handle(ctx).update(ctx, |settings, ctx| {
                    if let Err(e) = settings
                        .did_check_to_trigger_openwarp_launch_modal
                        .set_value(true, ctx)
                    {
                        log::warn!("Failed to mark OpenWarp launch modal as dismissed: {e}");
                    }
                });
            }
        });

        Self {
            is_openwarp_launch_modal_open: false,
            is_hoa_onboarding_open: false,
            target_window_id: None,
        }
    }

    pub fn target_window_id(&self) -> Option<WindowId> {
        self.target_window_id
    }

    pub fn is_openwarp_launch_modal_open(&self) -> bool {
        self.is_openwarp_launch_modal_open && self.target_window_id.is_some()
    }

    pub fn mark_openwarp_launch_modal_dismissed(&mut self, ctx: &mut ModelContext<Self>) {
        self.set_openwarp_launch_modal_open(false, ctx);
    }

    /// Returns whether the HOA onboarding flow is currently open.
    pub fn is_hoa_onboarding_open(&self) -> bool {
        self.is_hoa_onboarding_open && self.target_window_id.is_some()
    }

    pub fn mark_hoa_onboarding_dismissed(&mut self, ctx: &mut ModelContext<Self>) {
        self.set_hoa_onboarding_open(false, ctx);
    }

    /// Returns true if any one-time modal is currently open.
    pub fn is_any_modal_open(&self) -> bool {
        (self.is_openwarp_launch_modal_open || self.is_hoa_onboarding_open)
            && self.target_window_id.is_some()
    }

    #[cfg(debug_assertions)]
    pub fn force_open_openwarp_launch_modal(&mut self, ctx: &mut ModelContext<Self>) {
        self.set_openwarp_launch_modal_open(true, ctx);
    }

    pub fn update_target_window_id(&mut self, window_id: WindowId, ctx: &mut ModelContext<Self>) {
        let was_any_modal_visible = self.is_any_modal_open();
        self.target_window_id = Some(window_id);
        let is_any_modal_visible = self.is_any_modal_open();
        if was_any_modal_visible != is_any_modal_visible {
            ctx.emit(OneTimeModalEvent::VisibilityChanged {
                is_open: is_any_modal_visible,
            });
        }
    }

    fn set_openwarp_launch_modal_open(
        &mut self,
        is_open: bool,
        ctx: &mut ModelContext<Self>,
    ) -> bool {
        if self.is_openwarp_launch_modal_open != is_open {
            self.is_openwarp_launch_modal_open = is_open;
            ctx.emit(OneTimeModalEvent::VisibilityChanged { is_open });
            return true;
        }
        false
    }

    fn set_hoa_onboarding_open(&mut self, is_open: bool, ctx: &mut ModelContext<Self>) -> bool {
        if self.is_hoa_onboarding_open != is_open {
            self.is_hoa_onboarding_open = is_open;
            ctx.emit(OneTimeModalEvent::VisibilityChanged { is_open });
            return true;
        }
        false
    }

    fn check_and_trigger_all_modals(&mut self, ctx: &mut ModelContext<Self>) {
        // Never show one-time modals on WASM.
        if cfg!(target_family = "wasm") {
            return;
        }

        // Existing users should never see the code toolbelt new feature popup.
        CodeSettings::handle(ctx).update(ctx, |settings, ctx| {
            if let Err(e) = settings
                .dismissed_code_toolbelt_new_feature_popup
                .set_value(true, ctx)
            {
                log::warn!("Failed to mark code toolbelt new feature popup as dismissed: {e}");
            }
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OneTimeModalEvent {
    VisibilityChanged { is_open: bool },
}

impl Entity for OneTimeModalModel {
    type Event = OneTimeModalEvent;
}

impl SingletonEntity for OneTimeModalModel {}
