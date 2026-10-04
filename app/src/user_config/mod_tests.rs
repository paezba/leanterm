use std::io::Write;
use std::path::Path;

use super::*;

#[cfg(feature = "local_fs")]
#[test]
fn test_default_tab_configs_dir_uses_underscores() {
    assert!(default_tab_configs_dir().ends_with("default_tab_configs"));
}

#[cfg(feature = "local_fs")]
#[test]
fn test_sanitize_toml_base_name_replaces_spaces_and_dots() {
    assert_eq!(sanitize_toml_base_name("My Project.v2"), "my_project_v2");
}

#[cfg(feature = "local_fs")]
#[test]
fn test_sanitize_toml_base_name_falls_back_for_empty_result() {
    assert_eq!(sanitize_toml_base_name("..."), "worktree");
}

fn write_tab_config_toml(dir: &Path, file_name: &str, config_name: &str) {
    let path = dir.join(file_name);
    let mut f = std::fs::File::create(path).unwrap();
    write!(f, "name = \"{}\"", config_name).unwrap();
}

#[cfg(feature = "local_fs")]
#[test]
fn test_load_tab_configs_deterministic_tie_breaking() {
    let dir = tempfile::tempdir().unwrap();
    write_tab_config_toml(dir.path(), "upper.toml", "Alpha");
    write_tab_config_toml(dir.path(), "lower.toml", "alpha");

    let (configs, errors) = load_tab_configs(dir.path());

    assert!(errors.is_empty());
    let names: Vec<&str> = configs.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["Alpha", "alpha"]);
}

#[cfg(feature = "local_fs")]
#[test]
fn test_load_tab_configs_empty_directory() {
    let dir = tempfile::tempdir().unwrap();

    let (configs, errors) = load_tab_configs(dir.path());

    assert!(configs.is_empty());
    assert!(errors.is_empty());
}

#[cfg(feature = "local_fs")]
#[test]
fn test_load_tab_configs_skips_non_toml_files() {
    let dir = tempfile::tempdir().unwrap();
    write_tab_config_toml(dir.path(), "real.toml", "Real");
    std::fs::write(dir.path().join("readme.md"), "not a config").unwrap();
    std::fs::write(dir.path().join("data.json"), "{}").unwrap();

    let (configs, errors) = load_tab_configs(dir.path());

    assert!(errors.is_empty());
    assert_eq!(configs.len(), 1);
    assert_eq!(configs[0].name, "Real");
}
