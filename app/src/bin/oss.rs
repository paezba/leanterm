// On Windows, we don't want to display a console window when the application is running in release
// builds. See https://doc.rust-lang.org/reference/runtime.html#the-windows_subsystem-attribute.
#![cfg_attr(feature = "release_bundle", windows_subsystem = "windows")]

use anyhow::Result;
use warp_core::AppId;
use warp_core::channel::{Channel, ChannelConfig, ChannelState, OzConfig, WarpServerConfig};
use warp_core::features::FeatureFlag;

/// AI, cloud, account, and onboarding features turned off to keep this build a lean terminal.
const LEAN_TERMINAL_DISABLED_FLAGS: &[FeatureFlag] = &[
    FeatureFlag::AgentMode,
    FeatureFlag::AgentOnboarding,
    FeatureFlag::AccountFirstOnboarding,
    FeatureFlag::HOAOnboardingFlow,
    FeatureFlag::AgentTips,
    FeatureFlag::OzLaunchModal,
    FeatureFlag::OpenWarpLaunchModal,
    FeatureFlag::OrchestrationLaunchModal,
    FeatureFlag::AgentCliLaunchModal,
    FeatureFlag::CodeLaunchModal,
    FeatureFlag::OzChangelogUpdates,
    FeatureFlag::GetStartedTab,
    FeatureFlag::CreateProjectFlow,
    FeatureFlag::AvatarInTabBar,
    FeatureFlag::ViewingSharedSessions,
    FeatureFlag::CreatingSharedSessions,
    FeatureFlag::SharedWithMe,
    FeatureFlag::AgentSharedSessions,
    FeatureFlag::CloudMode,
    FeatureFlag::CloudConversations,
    FeatureFlag::CloudEnvironments,
    FeatureFlag::WarpManagedSecrets,
    FeatureFlag::AmbientAgentsCommandLine,
    FeatureFlag::ScheduledAmbientAgents,
    FeatureFlag::McpServer,
    FeatureFlag::FileBasedMcp,
    FeatureFlag::UsageBasedPricing,
    FeatureFlag::WarpPacks,
    FeatureFlag::AgentManagementView,
    FeatureFlag::GithubPrPromptChip,
    FeatureFlag::AgentToolbarEditor,
    FeatureFlag::FigmaDetection,
    FeatureFlag::SuperGrok,
    FeatureFlag::ChatGPTSubscription,
    FeatureFlag::GeminiEnterprise,
    FeatureFlag::SoloUserByok,
    FeatureFlag::TeamApiKeys,
    FeatureFlag::APIKeyManagement,
];

// Simple wrapper around warp::run() for Warp OSS builds.
fn main() -> Result<()> {
    let mut state = ChannelState::new(
        Channel::Oss,
        ChannelConfig {
            app_id: AppId::new("dev", "warp", "WarpOss"),
            logfile_name: "warp-oss.log".into(),
            server_config: WarpServerConfig::production(),
            oz_config: OzConfig::production(),
            telemetry_config: None,
            crash_reporting_config: None,
            autoupdate_config: None,
            mcp_static_config: None,
        },
    )
    .with_additional_features(&[FeatureFlag::LeanTerminal])
    .with_disabled_features(LEAN_TERMINAL_DISABLED_FLAGS);
    if cfg!(debug_assertions) {
        state = state.with_additional_features(warp_core::features::DEBUG_FLAGS);
    }
    ChannelState::set(state);

    warp::run()
}

// If we're not using an external plist, embed the following as the Info.plist.
#[cfg(all(not(feature = "extern_plist"), target_os = "macos"))]
embed_plist::embed_info_plist_bytes!(r#"
    <?xml version="1.0" encoding="UTF-8"?>
    <!DOCTYPE plist PUBLIC "-//Apple Computer//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
    <plist version="1.0">
    <dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleDisplayName</key>
    <string>WarpOss</string>
    <key>CFBundleExecutable</key>
    <string>warp-oss</string>
    <key>CFBundleIdentifier</key>
    <string>dev.warp.WarpOss</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>WarpOss</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.developer-tools</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>UIDesignRequiresCompatibility</key>
    <true/>
    <key>CFBundleURLTypes</key>
    <array><dict><key>CFBundleURLName</key><string>Custom App</string><key>CFBundleURLSchemes</key><array><string>warposs</string></array></dict></array>
    <key>NSHumanReadableCopyright</key>
    <string>© 2026, Denver Technologies, Inc</string>
    </dict>
    </plist>
"#.as_bytes());
