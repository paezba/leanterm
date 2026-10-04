//! Integration tests for settings sidebar navigation and search.
//!
//! These pin down the user-visible behavior of the settings nav rail —
//! clicking rows, arrow-key cycling, umbrella expand/collapse, and search
//! filtering across both top-level pages and umbrella subpages — so that
//! refactors of the settings page model cannot silently regress them.

use warp::integration_testing::settings::{
    assert_settings_nav_page_visible, assert_settings_section, assert_umbrella_expanded,
    clear_settings_search, open_settings_page, type_settings_search,
};
use warp::integration_testing::terminal::wait_until_bootstrapped_single_pane_for_tab;
use warp::settings_view::SettingsSection;

use super::{Builder, new_builder};

/// Label of the umbrella that groups the agent subpages.
const AGENTS_UMBRELLA: &str = "Agents";

// ---------------------------------------------------------------------------
// Mouse navigation
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Keyboard navigation
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Search filtering
// ---------------------------------------------------------------------------

/// A query that only matches a top-level page hides the other top-level rows
/// and moves the selection onto the surviving page.
pub fn test_settings_search_filters_top_level_pages() -> Builder {
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(open_settings_page(SettingsSection::Account))
        .with_step(type_settings_search("keyboard shortcut"))
        .with_step(assert_settings_nav_page_visible(
            SettingsSection::Keybindings,
            true,
        ))
        .with_step(assert_settings_nav_page_visible(
            SettingsSection::About,
            false,
        ))
        // Account no longer matches, so the selection follows the filter.
        .with_step(assert_settings_section(SettingsSection::Keybindings))
}

/// Clearing the search restores the umbrella expansion state the user had
/// before searching, rather than leaving auto-expanded umbrellas open.
pub fn test_settings_search_clear_restores_umbrella_state() -> Builder {
    new_builder()
        .with_step(wait_until_bootstrapped_single_pane_for_tab(0))
        .with_step(open_settings_page(SettingsSection::Account))
        .with_step(assert_umbrella_expanded(AGENTS_UMBRELLA, false))
        .with_step(type_settings_search("codex"))
        .with_step(assert_umbrella_expanded(AGENTS_UMBRELLA, true))
        .with_step(clear_settings_search())
        .with_step(assert_umbrella_expanded(AGENTS_UMBRELLA, false))
        .with_step(assert_settings_nav_page_visible(
            SettingsSection::About,
            true,
        ))
}

// ---------------------------------------------------------------------------
// MCP servers
// ---------------------------------------------------------------------------
