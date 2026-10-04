use self::parse_url_paths::{WarpWebLink, get_item_data_from_warp_link};
use super::*;
use crate::ChannelState;
use crate::launch_configs::launch_config::make_mock_single_window_launch_config;

#[test]
fn test_find_matching_config() {
    let mut configs: Vec<LaunchConfig> = vec![];
    for i in 0..5 {
        add_mock_config_with_name(
            (String::from("config") + i.to_string().as_str()).as_str(),
            &mut configs,
        );
    }

    let with_extension = "config1.yaml";
    assert_eq!(
        find_matching_config(with_extension, &configs),
        Some(&configs[1])
    );

    let no_extension = "config4";
    assert_eq!(
        find_matching_config(no_extension, &configs),
        Some(&configs[4])
    );

    let caps_insensitive = "ConFig3";
    assert_eq!(
        find_matching_config(caps_insensitive, &configs),
        Some(&configs[3])
    );

    let missing_config = "missing";
    assert_eq!(find_matching_config(missing_config, &configs), None);
}

#[test]
fn test_find_matching_config_with_spaces() {
    let mut configs: Vec<LaunchConfig> = vec![];
    for i in 0..3 {
        add_mock_config_with_name(
            (String::from("config") + i.to_string().as_str()).as_str(),
            &mut configs,
        );
    }

    let with_space = "config 3.yaml";
    add_mock_config_with_name(with_space, &mut configs);
    assert_eq!(
        find_matching_config(with_space, &configs),
        Some(&configs[3])
    );

    let more_space = " a ";
    add_mock_config_with_name(more_space, &mut configs);
    assert_eq!(
        find_matching_config(more_space, &configs),
        Some(&configs[4])
    );
}

#[test]
fn test_find_matching_configs_special_chars() {
    let mut configs: Vec<LaunchConfig> = vec![];
    for i in 0..3 {
        add_mock_config_with_name(
            (String::from("config") + i.to_string().as_str()).as_str(),
            &mut configs,
        );
    }

    // test special characters
    let special_ascii = "yes! this_works,too-even[braces}and(parens'.";
    add_mock_config_with_name(special_ascii, &mut configs);
    assert_eq!(
        find_matching_config(special_ascii, &configs),
        Some(&configs[3])
    );

    // test emojis
    let bread = "🍞";
    add_mock_config_with_name(bread, &mut configs);
    assert_eq!(find_matching_config(bread, &configs), Some(&configs[4]));
}

fn add_mock_config_with_name(name: &str, configs: &mut Vec<LaunchConfig>) {
    let mut new_config = make_mock_single_window_launch_config();
    new_config.name = name.to_string();
    new_config.windows[0].tabs[0].title = Some(String::from("First tab from config ") + name);
    configs.push(new_config);
}

#[test]
fn test_find_matching_tab_config() {
    let configs = vec![
        make_mock_tab_config("my tab", Some("/tab_configs/my_tab.toml")),
        make_mock_tab_config("Deploy", Some("/tab_configs/Deploy.yaml")),
        make_mock_tab_config("dotted", Some("/tab_configs/foo.bar.toml")),
        make_mock_tab_config("orphan", None),
    ];

    // Stem match without extension.
    assert_eq!(
        find_matching_tab_config("my_tab", configs.clone()).map(|c| c.name),
        Some(String::from("my tab")),
    );

    // Stem match with extension.
    assert_eq!(
        find_matching_tab_config("my_tab.toml", configs.clone()).map(|c| c.name),
        Some(String::from("my tab")),
    );

    // Case-insensitive match.
    assert_eq!(
        find_matching_tab_config("deploy", configs.clone()).map(|c| c.name),
        Some(String::from("Deploy")),
    );

    // Dotted stem resolves both with and without `.toml`.
    assert_eq!(
        find_matching_tab_config("foo.bar", configs.clone()).map(|c| c.name),
        Some(String::from("dotted")),
    );
    assert_eq!(
        find_matching_tab_config("foo.bar.toml", configs.clone()).map(|c| c.name),
        Some(String::from("dotted")),
    );

    // Miss returns None.
    assert!(find_matching_tab_config("unknown", configs.clone()).is_none());

    // Configs without a `source_path` never match.
    assert!(find_matching_tab_config("orphan", configs).is_none());
}

fn make_mock_tab_config(name: &str, source_path: Option<&str>) -> TabConfig {
    TabConfig {
        name: name.to_string(),
        title: None,
        color: None,
        panes: vec![],
        params: HashMap::new(),
        source_path: source_path.map(PathBuf::from),
    }
}

#[test]
fn test_get_launch_config_path() {
    assert_eq!(
        get_launch_config_path("/path/to/a/config"),
        Some(String::from("path/to/a/config")),
    );
    assert_eq!(
        get_launch_config_path("/hello%20world.yaml"),
        Some(String::from("hello world.yaml")),
    );
    assert_eq!(
        get_launch_config_path("/%3Bhello%20%23world!"),
        Some(String::from(";hello #world!")),
    );
    assert_eq!(
        get_launch_config_path("/yes%21%20this_works%2Ctoo-even%5Bbraces%7Dand%28parens%27."),
        Some(String::from("yes! this_works,too-even[braces}and(parens'."))
    );
    assert_eq!(
        get_launch_config_path("/%F0%9F%8D%9E"),
        Some(String::from("🍞"))
    );
    assert_eq!(
        get_launch_config_path("/..filename_.with_dots.."),
        Some(String::from("..filename_.with_dots.."))
    );
}

#[test]
fn test_get_launch_config_path_invalid() {
    assert_eq!(get_launch_config_path(""), None);
    assert_eq!(get_launch_config_path("/"), None);
    assert_eq!(get_launch_config_path("%2F"), None);
    assert_eq!(get_launch_config_path("/../outside"), None);
    assert_eq!(get_launch_config_path("/..%2Foutside"), None);
    assert_eq!(get_launch_config_path("/A/.."), None);
    assert_eq!(get_launch_config_path("/A/../B"), None);
    assert_eq!(get_launch_config_path("//absolute"), None);
    assert_eq!(get_launch_config_path("/%2Fabsolute sneaky"), None);
    assert_eq!(get_launch_config_path("//../very_bad/.."), None);
}

#[test]
fn test_remove_extension() {
    assert_eq!(remove_extension(""), None);
    assert_eq!(remove_extension(".yaml"), Some(""));
    assert_eq!(remove_extension(" .yaml"), Some(" "));
    assert_eq!(remove_extension("config.yaml"), Some("config"));
    assert_eq!(remove_extension("..yaml"), Some("."));
    assert_eq!(remove_extension("config"), None);
    assert_eq!(remove_extension("🍞.yaml"), Some("🍞"));
}

#[test]
fn test_warp_web_link_notebook() {
    assert_eq!(
        get_item_data_from_warp_link(
            &Url::parse(&format!(
                "{}/drive/notebook/Performance-Analysis-LkDlnAe34vfYD2JXsAkssc?focused_folder_id=test_uid00000000000123&invitee_email=test@example.com",
                ChannelState::server_root_url()
            ))
            .unwrap()
        ),
        Some(WarpWebLink::DriveObject(Box::new(OpenWarpDriveObjectArgs {
            object_type: ObjectType::Notebook,
server_id: ServerId::from_string_lossy("LkDlnAe34vfYD2JXsAkssc"),
            settings: OpenWarpDriveObjectSettings {
                focused_folder_id: Some(ServerId::from(123)),
                invitee_email: Some(String::from("test@example.com")),
            },
        })))
    );
}

#[test]
fn test_warp_web_link_session() {
    assert_eq!(
        get_item_data_from_warp_link(
            &Url::parse(&format!(
                "{}/session/317d0686-7a0b-4b67-806b-aaa3e9df501b?
                pwd=6f727249-af9f-4025-a240-59df40a4c64b",
                ChannelState::server_root_url()
            ))
            .unwrap()
        ),
        Some(WarpWebLink::Session)
    );
}

#[test]
fn test_warp_web_link_workflow() {
    assert_eq!(
        get_item_data_from_warp_link(
            &Url::parse(&format!(
                "{}/drive/workflow/Remove-all-stopped-docker-container-image-and-volumes-ZCJSkai2gpwTqpBFs5HOfZ",
                ChannelState::server_root_url()
            ))
            .unwrap()
        ),
        Some(WarpWebLink::DriveObject(Box::new(OpenWarpDriveObjectArgs {
            object_type: ObjectType::Workflow,
server_id: ServerId::from_string_lossy("ZCJSkai2gpwTqpBFs5HOfZ"),
            settings: OpenWarpDriveObjectSettings::default(),
        })))
    );
}

#[test]
fn test_warp_web_link_failure() {
    assert_eq!(
        get_item_data_from_warp_link(&Url::parse("https://google.com").unwrap()),
        None
    );
}
#[test]
fn test_app_web_link_rewrites_to_new_cloud_agent_conversation() {
    let url = Url::parse(&format!("{}/app", ChannelState::server_root_url())).unwrap();
    let intent = web_intent_parser::maybe_rewrite_web_url_to_intent(&url).unwrap();

    assert_eq!(
        intent.as_str(),
        format!(
            "{}://action/new_cloud_agent_conversation?source=web_home",
            ChannelState::url_scheme()
        )
    );
}

// `resolve_browser_url` is what both browser-URL write paths
// (`PaneGroup::focus` and the `JoinedSession` handler) delegate to, so
// testing it here covers both.

#[test]
fn resolve_browser_url_keeps_parent_conversation_view_when_child_pane_has_its_own_link() {
    let parent_url = Url::parse(&format!(
        "{}/conversation/parent-token",
        ChannelState::server_root_url()
    ))
    .unwrap();
    let child_session_url = Url::parse(&format!(
        "{}/session/317d0686-7a0b-4b67-806b-aaa3e9df501b",
        ChannelState::server_root_url()
    ))
    .unwrap();

    let resolved = browser_url_resolution::resolve_browser_url(
        Some(parent_url.clone()),
        Some(child_session_url),
        false,
    );

    assert_eq!(resolved, Some(parent_url));
}

#[test]
fn resolve_browser_url_keeps_parent_conversation_view_when_focused_pane_has_no_link() {
    let parent_url = Url::parse(&format!(
        "{}/conversation/parent-token",
        ChannelState::server_root_url()
    ))
    .unwrap();

    let resolved =
        browser_url_resolution::resolve_browser_url(Some(parent_url.clone()), None, false);

    assert_eq!(resolved, Some(parent_url));
}

#[test]
fn resolve_browser_url_uses_requested_url_outside_the_viewer() {
    let base_app_url = Url::parse(&format!("{}/app", ChannelState::server_root_url())).unwrap();
    let requested_url = Url::parse(&format!(
        "{}/session/317d0686-7a0b-4b67-806b-aaa3e9df501b",
        ChannelState::server_root_url()
    ))
    .unwrap();

    let resolved = browser_url_resolution::resolve_browser_url(
        Some(base_app_url),
        Some(requested_url.clone()),
        false,
    );

    assert_eq!(resolved, Some(requested_url));
}

#[test]
fn resolve_browser_url_falls_back_to_base_app_url_outside_the_viewer() {
    let current_url = Url::parse(&format!(
        "{}/drive/notebook/some-notebook?focused_folder_id=abc",
        ChannelState::server_root_url()
    ))
    .unwrap();

    let resolved = browser_url_resolution::resolve_browser_url(Some(current_url), None, false);

    assert_eq!(
        resolved,
        Some(Url::parse(&format!("{}/app", ChannelState::server_root_url())).unwrap())
    );
}

#[test]
fn resolve_browser_url_bypasses_the_guard_when_force_redirect_is_set() {
    let parent_url = Url::parse(&format!(
        "{}/conversation/parent-token",
        ChannelState::server_root_url()
    ))
    .unwrap();
    let login_url = Url::parse(&format!("{}/login", ChannelState::server_root_url())).unwrap();

    let resolved = browser_url_resolution::resolve_browser_url(
        Some(parent_url),
        Some(login_url.clone()),
        true,
    );

    assert_eq!(resolved, Some(login_url));
}

#[test]
fn resolve_browser_url_returns_none_when_neither_url_is_known() {
    let resolved = browser_url_resolution::resolve_browser_url(None, None, false);

    assert_eq!(resolved, None);
}









#[test]
fn test_action_open_file_editor_parse_expands_home_dir() {
    let url = Url::parse(&format!(
        "{}://action/open_file_editor?path=~/tmp/test.rs&line=1",
        ChannelState::url_scheme()
    ))
    .unwrap();

    let action = Action::parse(&url).unwrap();
    match action {
        Action::OpenFileEditor { path, line_col } => {
            assert_eq!(
                path,
                PathBuf::from(shellexpand::tilde("~/tmp/test.rs").into_owned())
            );
            assert_eq!(
                line_col,
                Some(LineAndColumnArg {
                    line_num: 1,
                    column_num: None,
                })
            );
        }
        _ => panic!("unexpected action: {action:?}"),
    }
}

#[test]
fn test_action_open_file_editor_parse_requires_path() {
    let url = Url::parse(&format!(
        "{}://action/open_file_editor?line=1",
        ChannelState::url_scheme()
    ))
    .unwrap();

    assert!(Action::parse(&url).is_err());
}

#[test]
fn test_action_open_file_editor_parse_rejects_relative_path() {
    let url = Url::parse(&format!(
        "{}://action/open_file_editor?path=src/main.rs&line=1",
        ChannelState::url_scheme()
    ))
    .unwrap();

    assert!(Action::parse(&url).is_err());
}

#[test]
fn test_action_open_file_editor_parse_rejects_column_without_line() {
    let url = Url::parse(&format!(
        "{}://action/open_file_editor?path=/tmp/test.rs&column=8",
        ChannelState::url_scheme()
    ))
    .unwrap();

    assert!(Action::parse(&url).is_err());
}

#[test]
fn test_action_open_file_editor_parse_rejects_invalid_line_or_column() {
    let invalid_line = Url::parse(&format!(
        "{}://action/open_file_editor?path=/tmp/test.rs&line=abc",
        ChannelState::url_scheme()
    ))
    .unwrap();
    assert!(Action::parse(&invalid_line).is_err());

    let zero_line = Url::parse(&format!(
        "{}://action/open_file_editor?path=/tmp/test.rs&line=0",
        ChannelState::url_scheme()
    ))
    .unwrap();
    assert!(Action::parse(&zero_line).is_err());

    let invalid_column = Url::parse(&format!(
        "{}://action/open_file_editor?path=/tmp/test.rs&line=1&column=0",
        ChannelState::url_scheme()
    ))
    .unwrap();
    assert!(Action::parse(&invalid_column).is_err());
}












// -- handle_incoming_uri validation errors -----------------------------------


#[test]
fn test_parse_tab_path_expands_tilde() {
    let url = Url::parse("warp://action/new_tab?path=~/Projects").unwrap();
    let home = dirs::home_dir().expect("HOME must be set for this test");
    assert_eq!(parse_tab_path(&url), Some(home.join("Projects")));
}


#[test]
fn test_parse_tab_path_absolute_path_unchanged() {
    let url = Url::parse("warp://action/new_tab?path=/tmp/foo").unwrap();
    assert_eq!(parse_tab_path(&url), Some(PathBuf::from("/tmp/foo")));
}


#[test]
fn test_parse_tab_path_missing_returns_none() {
    let url = Url::parse("warp://action/new_tab").unwrap();
    assert_eq!(parse_tab_path(&url), None);
}


// -- warp://settings deeplink parsing ----------------------------------------



// -- post-checkout desktop hand-off ------------------------------------------


// Regression coverage for issue #9005: shell scripts opened via `file://` should run,
// not open in the editor. Exercised through the pure routing helper to avoid standing
// up a full `AppContext`.

#[test]
#[cfg(unix)]
fn test_open_file_executable_sh_routes_to_execute() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("run.sh");
    std::fs::write(&p, b"#!/bin/sh\n:\n").unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    let action = classify_open_file_action(&p, true);
    assert_eq!(action, OpenFileAction::ExecuteInSession);
}

#[test]
#[cfg(unix)]
fn test_open_file_non_executable_sh_routes_to_editor() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("view.sh");
    std::fs::write(&p, b"#!/bin/sh\n:\n").unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(classify_open_file_action(&p, true), OpenFileAction::Editor);
}

#[test]
#[cfg(unix)]
fn test_open_file_executable_bash_zsh_fish_route_to_execute() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    for name in ["run.bash", "run.zsh", "run.fish", "run.command"] {
        let p = dir.path().join(name);
        std::fs::write(&p, b"#!/bin/sh\n:\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            classify_open_file_action(&p, true),
            OpenFileAction::ExecuteInSession,
            "{name} should route to ExecuteInSession",
        );
    }
}

#[test]
fn test_open_file_markdown_routes_to_notebook_when_viewer_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("README.md");
    std::fs::write(&p, b"# hi\n").unwrap();
    assert_eq!(
        classify_open_file_action(&p, true),
        OpenFileAction::Notebook
    );
}

#[test]
fn test_open_file_markdown_routes_to_editor_when_viewer_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("README.md");
    std::fs::write(&p, b"# hi\n").unwrap();
    assert_eq!(classify_open_file_action(&p, false), OpenFileAction::Editor);
}

#[test]
fn test_open_file_ipynb_routes_to_notebook_when_enabled() {
    // A `.ipynb` opened via `file://` (e.g. "Open with Warp" from Finder) opens
    // in the notebook viewer, not the raw-JSON code editor.
    let _flag = crate::features::FeatureFlag::JupyterNotebookRendering.override_enabled(true);
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("analysis.ipynb");
    std::fs::write(&p, b"{\"nbformat\": 4, \"cells\": []}\n").unwrap();
    assert_eq!(
        classify_open_file_action(&p, false),
        OpenFileAction::Notebook
    );
}

#[test]
fn test_open_file_ipynb_opens_in_editor_when_disabled() {
    // Without the feature flag, `.ipynb` is not rendered in the notebook viewer
    // and falls through to the code editor.
    let _flag = crate::features::FeatureFlag::JupyterNotebookRendering.override_enabled(false);
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("analysis.ipynb");
    std::fs::write(&p, b"{\"nbformat\": 4, \"cells\": []}\n").unwrap();
    assert_eq!(classify_open_file_action(&p, true), OpenFileAction::Editor);
}

#[test]
#[cfg(feature = "local_fs")]
fn test_open_file_rust_source_still_opens_in_editor() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("main.rs");
    std::fs::write(&p, b"fn main() {}\n").unwrap();
    assert_eq!(classify_open_file_action(&p, true), OpenFileAction::Editor);
}

#[test]
#[cfg(unix)]
fn test_open_file_editor_executable_sh_opens_in_editor() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("run.sh");
    std::fs::write(&p, b"#!/bin/sh\n:\n").unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(can_open_file_editor_path(&p));
}

#[test]
#[cfg(feature = "local_fs")]
fn test_open_file_editor_rust_source_opens_in_editor() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("main.rs");
    std::fs::write(&p, b"fn main() {}\n").unwrap();
    assert!(can_open_file_editor_path(&p));
}

#[test]
#[cfg(feature = "local_fs")]
fn test_open_file_editor_binary_file_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("image.png");
    std::fs::write(&p, b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR").unwrap();
    assert!(!can_open_file_editor_path(&p));
}

#[test]
fn test_open_file_directory_routes_to_session() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        classify_open_file_action(dir.path(), true),
        OpenFileAction::ExecuteInSession
    );
}


#[test]
fn test_session_uri_host_parsing() {
    let result = UriHost::from_str("session");
    assert!(matches!(result, Ok(UriHost::Session)));
}


#[test]
fn test_session_uri_empty_path_does_not_panic() {
    let url = Url::parse(&format!("{}://session/", ChannelState::url_scheme())).unwrap();
    let host = validate_custom_uri(&url).unwrap();
    assert!(matches!(host, UriHost::Session));
}

#[test]
fn test_session_uri_invalid_hex_does_not_panic() {
    let url = Url::parse(&format!(
        "{}://session/ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ",
        ChannelState::url_scheme()
    ))
    .unwrap();
    let host = validate_custom_uri(&url).unwrap();
    assert!(matches!(host, UriHost::Session));
}



#[test]
fn test_decode_uuid_hex_rejects_invalid_chars() {
    assert!(super::decode_uuid_hex("ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ").is_none());
}
