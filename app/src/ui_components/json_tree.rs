//! Generic, reusable JSON tree rendering component.
//!
//! Renders a `serde_json::Value` as an interactive, collapsible tree with
//! theme-driven colors.
use std::cell::RefCell;
use std::collections::HashMap;

use pathfinder_color::ColorU;
use warpui::elements::MouseStateHandle;


// ---------------------------------------------------------------------------
// Callback type aliases
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// PathSegment
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// JsonTreeState
// ---------------------------------------------------------------------------

impl JsonTreeState {

}

// ---------------------------------------------------------------------------
// JsonTreeColors
// ---------------------------------------------------------------------------

impl JsonTreeColors {
}

// ---------------------------------------------------------------------------
// Annotation helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Long-string helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "json_tree_tests.rs"]
mod tests;

/// Pre-resolved colors for each JSON value category, sourced from the active
/// `WarpTheme`. Build this once per render from `JsonTreeColors::from_theme`.
#[derive(Debug, Clone, Copy)]
pub struct JsonTreeColors {
    /// Color for object/array keys and array indices.
    pub key: ColorU,
    /// Color for string values.
    pub string: ColorU,
    /// Color for number values.
    pub number: ColorU,
    /// Color for boolean values.
    pub bool: ColorU,
    /// Color for null values.
    pub null: ColorU,
    /// Color for type/size annotations (`{} 4 keys`) and punctuation.
    pub annotation: ColorU,
}

/// Stores per-node expansion state for a rendered JSON tree.
///
/// State is keyed by `Vec<PathSegment>` which identifies each node by its
/// structural path in the tree. Path-keyed state is stable across
/// streaming re-parses because the path for any given node is deterministic
/// as long as the surrounding JSON structure is unchanged.
#[derive(Debug, Default, Clone)]
pub struct JsonTreeState {
    /// Expansion state for object/array nodes. Absent = expanded at depth 0,
    /// collapsed at depth 1+. An explicit entry always takes precedence.
    node_expansion: HashMap<Vec<PathSegment>, bool>,
    /// Expansion state for long string values. Absent = collapsed (elided).
    string_expansion: HashMap<Vec<PathSegment>, bool>,
    /// Per-node `MouseStateHandle`s used by the `Hoverable` elements in each
    /// rendered row.
    ///
    /// WarpUI requires that `MouseStateHandle`s be created once and reused
    /// across renders: creating `MouseStateHandle::default()` inline during
    /// render discards the `click_count` set during `LeftMouseDown` before
    /// `LeftMouseUp` fires, so click handlers never trigger. Storing handles
    /// here (keyed by node path) gives each row a stable handle that persists
    /// across re-renders triggered by `ctx.notify()`.
    ///
    /// A single map covers both container nodes and long-string rows. The same
    /// path cannot appear in both roles at once — a node is either a container
    /// or a scalar string, never both — so one handle per path is sufficient.
    mouse_states: RefCell<HashMap<Vec<PathSegment>, MouseStateHandle>>,
}

/// A single segment of a path into a `serde_json::Value` tree.
///
/// A sequence of segments uniquely identifies any node in the tree by its
/// structural position (key in an object, index in an array).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PathSegment {
    /// A named key in a JSON object.
    Key(String),
    /// A 0-based index in a JSON array.
    Index(usize),
}
