use std::collections::HashSet;

use leanterm_core::settings::SupportedPlatforms;
use leanterm_core::settings::macros::define_settings_group;

use crate::banner::BannerState;

define_settings_group!(GeneralSettings, settings: [
    show_warning_before_quitting: ShowWarningBeforeQuitting {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::DESKTOP,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "general.show_warning_before_quitting",
        description: "Whether to show a warning dialog before quitting Leanterm.",
    },
    quit_on_last_window_closed: QuitOnLastWindowClosed {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::MAC,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "general.quit_on_last_window_closed",
        description: "Whether to quit Leanterm when the last window is closed.",
    },
    restore_session: RestoreSession {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::DESKTOP,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "general.restore_session",
        description: "Whether to restore the previous session when Leanterm starts up.",
    },
    add_app_as_login_item: LoginItem {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::OR(
            Box::new(SupportedPlatforms::MAC),
            Box::new(SupportedPlatforms::WINDOWS),
        ),
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "general.login_item",
        description: "Whether to launch Leanterm automatically when you log in.",
    },
    // Records whether the app has been added as a login item.
    // If it has, we don't try to add it again unless the user explicitly
    // retoggles the setting. This is to allow a user to remove the login item
    // directly from their OS's startup UI and not have it re-added when they
    // next start Leanterm.
    app_added_as_login_item: AppAddedAsLoginItem {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::OR(
            Box::new(SupportedPlatforms::MAC),
            Box::new(SupportedPlatforms::WINDOWS),
        ),
        surface: settings::SettingSurfaces::GUI,
        private: true,
    },
    link_tooltip: LinkTooltip {
        type: bool,
        default: true,
        supported_platforms: SupportedPlatforms::ALL,
        surface: settings::SettingSurfaces::GUI,
        private: false,
        toml_path: "general.link_tooltip",
        description: "Whether to show a tooltip when hovering over links.",
    },
    agent_mode_onboarding_block_shown: AgentModeOnboardingBlockShown {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        surface: settings::SettingSurfaces::GUI,
        private: true,
    },
    telemetry_banner_dismissed: TelemetryBannerDismissed {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        surface: settings::SettingSurfaces::GUI,
        private: true,
    },
    user_default_shell_unsupported_banner_state: UserDefaultShellUnsupportedBannerState {
        type: BannerState,
        default: BannerState::default(),
        supported_platforms: SupportedPlatforms::ALL,
        surface: settings::SettingSurfaces::GUI,
        private: true,
    },
    open_in_leanterm_banner_dismissed_for_markdown: OpenInLeantermBannerDismissedMarkdown {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        surface: settings::SettingSurfaces::GUI,
        private: true,
    },
    open_in_leanterm_banner_dismissed_for_code_and_text: OpenInLeantermBannerDismissedCode {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        surface: settings::SettingSurfaces::GUI,
        private: true,
    },
    did_non_anonymous_user_log_in: DidNonAnonymousUserLogIn {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        surface: settings::SettingSurfaces::GUI,
        private: true,
    },
    build_plan_migration_modal_dismissed: BuildPlanMigrationModalDismissed {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        surface: settings::SettingSurfaces::GUI,
        private: true,
    },
    // One-time flag tracking whether the OpenLeanterm launch modal has already been
    // shown to the user. Not user-visible; modeled as a setting so it's only
    // shown once per user regardless of the number of devices they use.
    did_check_to_trigger_openleanterm_launch_modal: DidShowOpenLeantermLaunchModal {
        type: bool,
        default: false,
        supported_platforms: SupportedPlatforms::ALL,
        surface: settings::SettingSurfaces::GUI,
        private: true,
    },
    bonus_grants_shown: BonusGrantsShown {
        type: HashSet<String>,
        default: HashSet::new(),
        supported_platforms: SupportedPlatforms::ALL,
        surface: settings::SettingSurfaces::GUI,
        private: true,
    },
]);
