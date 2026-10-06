use std::path::Path;

use anyhow::Result;
use toml_edit::{DocumentMut, Item, Table, TableLike, Value};

/// Renames keys that the settings file still has under the app's former brand name (for example
/// `[warpify.ssh]` becomes `[leantermify.ssh]`).
///
/// Returns whether the file was rewritten. A missing file is not an error.
pub fn migrate_legacy_setting_keys(path: &Path) -> Result<bool> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Ok(false);
    };
    let mut document: DocumentMut = contents.parse()?;
    if !rename_keys(document.as_table_mut()) {
        return Ok(false);
    }
    std::fs::write(path, document.to_string())?;
    Ok(true)
}

fn rename_keys(table: &mut dyn TableLike) -> bool {
    let mut changed = false;
    for (_, item) in table.iter_mut() {
        changed |= rename_nested_keys(item);
    }

    let legacy_keys: Vec<String> = table
        .iter()
        .map(|(key, _)| key.to_owned())
        .filter(|key| renamed_key(key) != *key)
        .collect();
    for legacy_key in legacy_keys {
        let new_key = renamed_key(&legacy_key);
        let Some(item) = table.remove(&legacy_key) else {
            continue;
        };
        // A value already saved under the new name wins over the legacy one.
        if table.get(&new_key).is_none() {
            table.insert(&new_key, item);
        }
        changed = true;
    }
    changed
}

fn rename_nested_keys(item: &mut Item) -> bool {
    match item {
        Item::Table(table) => rename_keys(table),
        Item::Value(Value::InlineTable(table)) => rename_keys(table),
        Item::ArrayOfTables(tables) => {
            tables.iter_mut().fold(false, |changed, table: &mut Table| {
                rename_keys(table) | changed
            })
        }
        Item::None | Item::Value(_) => false,
    }
}

fn renamed_key(key: &str) -> String {
    key.replace("warp", "leanterm").replace("Warp", "Leanterm")
}

#[cfg(test)]
#[path = "legacy_keys_tests.rs"]
mod tests;
