//! Tests for async find functionality.

use std::sync::Arc;

use super::{
    AbsoluteMatch, AsyncFindConfig, AsyncFindStatus, BlockFindResults, is_query_refinement,
};
use crate::terminal::block_list_element::GridType;
use crate::terminal::find::model::FindOptions;
use crate::terminal::model::grid::grid_handler::AbsolutePoint;
use crate::terminal::model::terminal_model::{BlockIndex, BlockSortDirection};

/// Helper to create an AbsoluteMatch at a given row with default column span.
fn make_match(row: u64) -> AbsoluteMatch {
    AbsoluteMatch {
        start: AbsolutePoint { row, col: 0 },
        end: AbsolutePoint { row, col: 5 },
        is_filtered: false,
    }
}

/// Helper to create an AbsoluteMatch at a given row and column range.
fn make_match_at(row: u64, start_col: usize, end_col: usize) -> AbsoluteMatch {
    AbsoluteMatch {
        start: AbsolutePoint {
            row,
            col: start_col,
        },
        end: AbsolutePoint { row, col: end_col },
        is_filtered: false,
    }
}

#[test]
fn test_block_invalidation_with_dirty_range() {
    // Test that dirty range invalidation merges correctly with existing matches.
    let mut results = BlockFindResults::default();
    let block_index = BlockIndex(0);
    let grid_type = GridType::Output;

    // Seed with matches at absolute rows 5, 15, 25.
    results.terminal_matches.insert(
        (block_index, grid_type),
        vec![
            make_match_at(5, 0, 2),
            make_match_at(15, 0, 2),
            make_match_at(25, 0, 2),
        ],
    );

    // Dirty range 10..=20 overlaps with match at row 15.
    // New matches found in dirty range: rows 12 and 18.
    let new_matches = vec![make_match_at(12, 0, 2), make_match_at(18, 0, 2)];
    results.update_dirty_matches(block_index, grid_type, 10..=20, new_matches);

    let stored = results
        .terminal_matches
        .get(&(block_index, grid_type))
        .unwrap();

    // Should have: 5, 12, 18, 25 (match at 15 was replaced).
    assert_eq!(stored.len(), 4);
    assert_eq!(stored[0].start_row(), 5);
    assert_eq!(stored[1].start_row(), 12);
    assert_eq!(stored[2].start_row(), 18);
    assert_eq!(stored[3].start_row(), 25);
}

#[test]
fn test_is_query_refinement() {
    assert!(is_query_refinement("hel", "hello"));
    assert!(is_query_refinement("foo", "foobar"));
    assert!(!is_query_refinement("hello", "hel"));
    assert!(!is_query_refinement("hello", "hello"));
    assert!(!is_query_refinement("bar", "foo"));
    assert!(!is_query_refinement("", "hello"));
}

#[test]
fn test_async_find_config_from_options() {
    // Empty query should return None.
    let options = FindOptions::default();
    assert!(AsyncFindConfig::from_options(&options, BlockSortDirection::MostRecentLast).is_none());

    // Query with only whitespace should return None.
    let options = FindOptions {
        query: Some(Arc::new("   ".to_string())),
        ..Default::default()
    };
    assert!(AsyncFindConfig::from_options(&options, BlockSortDirection::MostRecentLast).is_none());

    // Valid query should return Some config.
    let options = FindOptions {
        query: Some(Arc::new("hello".to_string())),
        is_case_sensitive: true,
        is_regex_enabled: false,
        blocks_to_include_in_results: Some(vec![BlockIndex(0), BlockIndex(1)]),
    };
    let config = AsyncFindConfig::from_options(&options, BlockSortDirection::MostRecentFirst);
    assert!(config.is_some());
    let config = config.unwrap();
    assert_eq!(config.query.as_str(), "hello");
    assert!(config.is_case_sensitive);
    assert!(!config.is_regex_enabled);
    assert_eq!(
        config.blocks_to_include,
        Some(vec![BlockIndex(0), BlockIndex(1)])
    );
}

#[test]
fn test_block_find_results_total_count() {
    let mut results = BlockFindResults::default();
    assert_eq!(results.total_match_count(), 0);

    // Add some terminal matches.
    results
        .terminal_matches
        .entry((BlockIndex(0), GridType::Output))
        .or_default()
        .push(make_match(0));
    assert_eq!(results.total_match_count(), 1);

    // Add more terminal matches.
    results
        .terminal_matches
        .entry((BlockIndex(0), GridType::Output))
        .or_default()
        .push(make_match(1));
    results
        .terminal_matches
        .entry((BlockIndex(1), GridType::PromptAndCommand))
        .or_default()
        .push(make_match(0));
    assert_eq!(results.total_match_count(), 3);
}

#[test]
fn test_block_find_results_remove_block() {
    let mut results = BlockFindResults::default();

    // Add matches for block 0 and block 1.
    results
        .terminal_matches
        .entry((BlockIndex(0), GridType::Output))
        .or_default()
        .push(make_match(0));
    results
        .terminal_matches
        .entry((BlockIndex(0), GridType::PromptAndCommand))
        .or_default()
        .push(make_match(0));
    results
        .terminal_matches
        .entry((BlockIndex(1), GridType::Output))
        .or_default()
        .push(make_match(0));
    assert_eq!(results.total_match_count(), 3);

    // Remove block 0.
    results.remove_block(BlockIndex(0));
    assert_eq!(results.total_match_count(), 1);

    // Block 1 should still have its matches.
    assert!(
        results
            .terminal_matches
            .contains_key(&(BlockIndex(1), GridType::Output))
    );
}

#[test]
fn test_async_find_status_display() {
    assert_eq!(format!("{}", AsyncFindStatus::Idle), "Idle");
    assert_eq!(format!("{}", AsyncFindStatus::Complete), "Complete");
    assert_eq!(format!("{}", AsyncFindStatus::Scanning), "Scanning");
}

#[test]
fn test_absolute_match_is_truncated() {
    let match_at_row_5 = make_match(5);
    // Not truncated when num_lines_truncated <= start row.
    assert!(!match_at_row_5.is_truncated(0));
    assert!(!match_at_row_5.is_truncated(5));
    // Truncated when num_lines_truncated > start row.
    assert!(match_at_row_5.is_truncated(6));
    assert!(match_at_row_5.is_truncated(100));
}

#[test]
fn test_update_dirty_matches_empty_existing() {
    let mut results = BlockFindResults::default();
    let block_index = BlockIndex(0);
    let grid_type = GridType::Output;

    // Update with new matches when there are no existing matches.
    let new_matches = vec![make_match(5), make_match(10), make_match(15)];
    results.update_dirty_matches(block_index, grid_type, 5..=15, new_matches.clone());

    let stored = results
        .terminal_matches
        .get(&(block_index, grid_type))
        .unwrap();
    assert_eq!(stored.len(), 3);
    assert_eq!(stored[0].start_row(), 5);
    assert_eq!(stored[1].start_row(), 10);
    assert_eq!(stored[2].start_row(), 15);
}

#[test]
fn test_update_dirty_matches_prepend() {
    let mut results = BlockFindResults::default();
    let block_index = BlockIndex(0);
    let grid_type = GridType::Output;

    // Seed with matches at rows 20, 30.
    results.terminal_matches.insert(
        (block_index, grid_type),
        vec![make_match(20), make_match(30)],
    );

    // Update with dirty range before all existing matches.
    let new_matches = vec![make_match(5), make_match(10)];
    results.update_dirty_matches(block_index, grid_type, 5..=10, new_matches);

    let stored = results
        .terminal_matches
        .get(&(block_index, grid_type))
        .unwrap();
    assert_eq!(stored.len(), 4);
    assert_eq!(stored[0].start_row(), 5);
    assert_eq!(stored[1].start_row(), 10);
    assert_eq!(stored[2].start_row(), 20);
    assert_eq!(stored[3].start_row(), 30);
}

#[test]
fn test_update_dirty_matches_append() {
    let mut results = BlockFindResults::default();
    let block_index = BlockIndex(0);
    let grid_type = GridType::Output;

    // Seed with matches at rows 5, 10.
    results.terminal_matches.insert(
        (block_index, grid_type),
        vec![make_match(5), make_match(10)],
    );

    // Update with dirty range after all existing matches.
    let new_matches = vec![make_match(20), make_match(30)];
    results.update_dirty_matches(block_index, grid_type, 20..=30, new_matches);

    let stored = results
        .terminal_matches
        .get(&(block_index, grid_type))
        .unwrap();
    assert_eq!(stored.len(), 4);
    assert_eq!(stored[0].start_row(), 5);
    assert_eq!(stored[1].start_row(), 10);
    assert_eq!(stored[2].start_row(), 20);
    assert_eq!(stored[3].start_row(), 30);
}

#[test]
fn test_update_dirty_matches_replace_middle() {
    let mut results = BlockFindResults::default();
    let block_index = BlockIndex(0);
    let grid_type = GridType::Output;

    // Seed with matches at rows 5, 15, 25.
    results.terminal_matches.insert(
        (block_index, grid_type),
        vec![make_match(5), make_match(15), make_match(25)],
    );

    // Update dirty range 10..=20, which overlaps with the match at row 15.
    // Replace it with matches at rows 12 and 18.
    let new_matches = vec![make_match(12), make_match(18)];
    results.update_dirty_matches(block_index, grid_type, 10..=20, new_matches);

    let stored = results
        .terminal_matches
        .get(&(block_index, grid_type))
        .unwrap();
    assert_eq!(stored.len(), 4);
    assert_eq!(stored[0].start_row(), 5);
    assert_eq!(stored[1].start_row(), 12);
    assert_eq!(stored[2].start_row(), 18);
    assert_eq!(stored[3].start_row(), 25);
}

#[test]
fn test_update_dirty_matches_clear_range() {
    let mut results = BlockFindResults::default();
    let block_index = BlockIndex(0);
    let grid_type = GridType::Output;

    // Seed with matches at rows 5, 15, 25.
    results.terminal_matches.insert(
        (block_index, grid_type),
        vec![make_match(5), make_match(15), make_match(25)],
    );

    // Update dirty range 10..=20 with no new matches (clears the match at row 15).
    results.update_dirty_matches(block_index, grid_type, 10..=20, vec![]);

    let stored = results
        .terminal_matches
        .get(&(block_index, grid_type))
        .unwrap();
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].start_row(), 5);
    assert_eq!(stored[1].start_row(), 25);
}
