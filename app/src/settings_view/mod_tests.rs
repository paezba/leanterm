use settings_page::{
    FilteredPageType, MatchData, PageType, SettingsWidget, search_terms_match,
};
use warpui::elements::Empty;
use warpui::{
    AppContext, Element, Entity, View,
};

use super::*;
use crate::appearance::Appearance;
use crate::workspaces::workspace::{BillingMetadata, CustomerType};

fn billing_metadata(customer_type: CustomerType) -> BillingMetadata {
    BillingMetadata {
        customer_type,
        ..Default::default()
    }
}

#[test]
fn paid_workspace_without_team_shows_only_workspace_badge() {
    let billing_metadata = billing_metadata(CustomerType::Enterprise);

    let presentation = plan_header_presentation(Some(&billing_metadata), false, false);

    assert_eq!(presentation.badge_label.as_deref(), Some("Enterprise"));
    assert!(!presentation.show_personal_upgrade);
}

#[test]
fn free_workspace_without_team_shows_free_badge_once() {
    let billing_metadata = billing_metadata(CustomerType::Free);

    let presentation = plan_header_presentation(Some(&billing_metadata), false, false);

    assert_eq!(presentation.badge_label.as_deref(), Some("Free"));
    assert!(presentation.show_personal_upgrade);
}

#[test]
fn paid_workspace_with_team_shows_only_workspace_badge() {
    let billing_metadata = billing_metadata(CustomerType::Enterprise);

    let presentation = plan_header_presentation(Some(&billing_metadata), true, false);

    assert_eq!(presentation.badge_label.as_deref(), Some("Enterprise"));
    assert!(!presentation.show_personal_upgrade);
}

#[test]
fn anonymous_account_shows_free_badge_once() {
    let presentation = plan_header_presentation(None, false, true);

    assert_eq!(presentation.badge_label.as_deref(), Some("Free"));
    assert!(presentation.show_personal_upgrade);
}

#[test]
fn signed_in_account_without_workspace_shows_free_badge_once() {
    let presentation = plan_header_presentation(None, false, false);

    assert_eq!(presentation.badge_label.as_deref(), Some("Free"));
    assert!(presentation.show_personal_upgrade);
}

// ── MatchData behavior ──────────────────────────────────────────────────────

#[test]
fn match_data_uncounted_true_is_truthy() {
    assert!(MatchData::Uncounted(true).is_truthy());
}

#[test]
fn match_data_uncounted_false_is_not_truthy() {
    assert!(!MatchData::Uncounted(false).is_truthy());
}

#[test]
fn match_data_countable_nonzero_is_truthy() {
    assert!(MatchData::Countable(3).is_truthy());
    assert!(MatchData::Countable(1).is_truthy());
}

#[test]
fn match_data_countable_zero_is_not_truthy() {
    assert!(!MatchData::Countable(0).is_truthy());
}

// ── Display labels ─────────────────────────────────────────────────


// ── slug / from_slug ───────────────────────────────────────────────

/// Every `SettingsSection` variant.
///
/// `all_sections_list_is_exhaustive` keeps this honest: adding a variant
/// breaks the exhaustive match there, which is the prompt to add it here.

/// Sections whose user-facing Display label has deliberately diverged from the
/// slug it was seeded from, because the slug is a stored contract that the
/// rename must not follow.



// ── current_stop_index ──────────────────────────────────────────────────────




// ── next_stop_index wrapping ────────────────────────────────────────────────

#[test]
fn next_stop_index_wraps_at_ends() {
    assert_eq!(next_stop_index(2, 3, CycleDirection::Down), 0);
    assert_eq!(next_stop_index(1, 3, CycleDirection::Up), 0);
    assert_eq!(next_stop_index(1, 3, CycleDirection::Down), 2);
}

#[test]
fn next_stop_index_handles_single_stop() {
    assert_eq!(next_stop_index(0, 1, CycleDirection::Up), 0);
}

// ── End-to-end cycling (no search) ──────────────────────────────────────────
// These tests simulate the sequence of nav-stop activations that would result
// from repeatedly pressing Down/Up, ensuring a collapsed umbrella is never
// skipped over.

// ── PageType filter lifecycle across a rebuild (APP-4922) ────────────────────
// Rebuilding a page's PageType resets its widget filter to every widget, so an
// active query has to be reapplied for only matching widgets to render. No page
// rebuilds itself on navigation any more (each subpage owns its own view), but
// these tests still pin the underlying PageType::Uncategorized filter lifecycle
// and the real search_terms_match predicate that the invariant rests on.


/// Minimal View so PageType<V> can be instantiated in a unit test without the
/// full SettingsView/ViewContext a real settings page requires.
struct TestSettingsView;

impl Entity for TestSettingsView {
    type Event = ();
}

impl View for TestSettingsView {
    fn ui_name() -> &'static str {
        "TestSettingsView"
    }

    fn render(&self, _: &AppContext) -> Box<dyn Element> {
        Empty::new().finish()
    }
}

/// A SettingsWidget whose only test-relevant state is its search terms; render
/// is never invoked by the filter lifecycle under test.
struct StubWidget {
    terms: &'static str,
}

impl SettingsWidget for StubWidget {
    type View = TestSettingsView;

    fn search_terms(&self) -> &str {
        self.terms
    }

    fn render(&self, _: &Self::View, _: &Appearance, _: &AppContext) -> Box<dyn Element> {
        Empty::new().finish()
    }
}


#[test]
fn search_terms_match_direct_unit_checks() {
    // All-words, case-insensitive, non-contiguous.
    assert!(search_terms_match(
        "active ai autosuggestions prompt",
        "autosuggestions"
    ));
    assert!(search_terms_match(
        "active ai autosuggestions prompt",
        "ACTIVE AI"
    ));
    assert!(search_terms_match(
        "file search fuzzy opener",
        "file search"
    ));
    // Every word must appear.
}



#[test]
fn reapply_handles_multi_word_and_case() {
    // A multi-word, case-insensitive query survives the rebuild + reapply cycle.
}


struct NeverRendersWidget {
    terms: &'static str,
}





/// An Uncategorized page with one widget plus a title trailing element.







/// Renders a `categorized_page_with_trailing` page, whose category has no subtitle (a
/// `render_sub_header` header, not `render_sub_header_with_description`).
struct CategoryHeaderTrailingElementTestView;
