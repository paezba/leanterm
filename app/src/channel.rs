pub use warp_core::channel::*;
use warp_core::features::FeatureFlag;

/// The product name shown in user-facing menus and pages.
pub fn product_name() -> &'static str {
    if FeatureFlag::LeanTerminal.is_enabled() {
        "WarpOss"
    } else {
        "Warp"
    }
}
