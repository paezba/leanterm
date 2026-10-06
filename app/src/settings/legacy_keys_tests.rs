use super::migrate_legacy_setting_keys;

#[test]
fn renames_legacy_tables_and_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    std::fs::write(
        &path,
        r#"[terminal.input]
warp_completions_enabled = false

[warpify.ssh]
enable_ssh_warpification = false
ssh_hosts_denylist = ["warp.example.com"]
"#,
    )
    .unwrap();

    assert!(migrate_legacy_setting_keys(&path).unwrap());

    let migrated: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    assert_eq!(
        migrated["terminal"]["input"]["leanterm_completions_enabled"].as_bool(),
        Some(false)
    );
    let ssh = &migrated["leantermify"]["ssh"];
    assert_eq!(ssh["enable_ssh_leantermification"].as_bool(), Some(false));
    assert_eq!(
        ssh["ssh_hosts_denylist"][0].as_str(),
        Some("warp.example.com"),
        "values must not be rewritten"
    );
    assert!(migrated.get("warpify").is_none());
}

#[test]
fn keeps_values_already_saved_under_the_new_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    std::fs::write(
        &path,
        "[code.editor]\nuse_warp_as_default_editor = false\nuse_leanterm_as_default_editor = true\n",
    )
    .unwrap();

    assert!(migrate_legacy_setting_keys(&path).unwrap());

    let migrated: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    let editor = migrated["code"]["editor"].as_table().unwrap();
    assert_eq!(
        editor["use_leanterm_as_default_editor"].as_bool(),
        Some(true)
    );
    assert_eq!(editor.len(), 1);
}

#[test]
fn leaves_current_files_and_missing_files_alone() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    assert!(!migrate_legacy_setting_keys(&path).unwrap());

    let contents = "[appearance.text]\nfont_size = 13.0\n";
    std::fs::write(&path, contents).unwrap();
    assert!(!migrate_legacy_setting_keys(&path).unwrap());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), contents);
}
