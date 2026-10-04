use std::collections::{HashMap, HashSet};
use std::iter;

use itertools::Itertools;
use session_sharing_protocol::common::{
    ParticipantId, ParticipantInfo, ParticipantList, ProfileData, Role, Sharer, Viewer,
};
use warpui::App;

use crate::auth::UserUid;
use crate::terminal::shared_session::presence_manager::{PRESET_COLORS, PresenceManager};

fn viewer_with_uid(uid: &str, is_present: bool) -> Viewer {
    Viewer {
        info: ParticipantInfo {
            profile_data: ProfileData {
                firebase_uid: uid.to_owned(),
                ..Default::default()
            },
            ..Default::default()
        },
        role: Role::Reader,
        is_present,
    }
}

#[test]
fn single_distinct_present_viewer_uid_filters_absent_duplicates() {
    let viewers = [
        viewer_with_uid("same", true),
        viewer_with_uid("same", true),
        viewer_with_uid("other", false),
    ];

    assert_eq!(
        PresenceManager::single_distinct_present_viewer_uid_from_viewers(viewers.iter()),
        Some("same")
    );
}

#[test]
fn single_distinct_present_viewer_uid_returns_none_for_zero_or_multiple_uids() {
    assert_eq!(
        PresenceManager::single_distinct_present_viewer_uid_from_viewers([].iter()),
        None
    );

    let viewers = [viewer_with_uid("one", true), viewer_with_uid("two", true)];
    assert_eq!(
        PresenceManager::single_distinct_present_viewer_uid_from_viewers(viewers.iter()),
        None
    );
}

#[test]
fn test_choosing_preset_colors() {
    App::test((), |mut app| async move {
        // Initialize with a sharer.
        let firebase_uid = UserUid::new("mock_firebase_uid");
        let presence_manager =
            app.add_model(|_| PresenceManager::new_for_sharer(ParticipantId::new(), firebase_uid));

        let sharer_id = ParticipantId::new();
        let sharer = Sharer {
            info: ParticipantInfo {
                id: sharer_id.clone(),
                profile_data: ProfileData {
                    ..Default::default()
                },
                ..Default::default()
            },
        };
        let mut viewers = Vec::new();
        let sharer_clone = sharer.clone();
        let viewers_clone = viewers.clone();

        presence_manager
            .update(&mut app, |presence_manager, ctx| {
                presence_manager.update_participants(
                    ParticipantList {
                        sharer: sharer_clone,
                        viewers: viewers_clone,
                        present_viewers: Default::default(),
                        absent_viewers: Default::default(),
                        guests: Default::default(),
                        pending_guests: Default::default(),
                    },
                    ctx,
                );
                let spawned_future = presence_manager
                    .load_participants_imgs_future_handle
                    .as_ref()
                    .expect("should have future handle");
                ctx.await_spawned_future(spawned_future.future_id())
            })
            .await;

        // We ourselves are the sharer, so no color is saved
        presence_manager.read(&app, |presence_manager: &PresenceManager, _ctx| {
            let sharer = presence_manager.get_sharer();
            assert!(sharer.is_none());

            let viewers = presence_manager.get_present_viewers().collect_vec();
            assert_eq!(viewers.len(), 0);
        });

        // Add new viewers one-by-one. Each new viewer should take the next preset color, while existing viewers keep their colors.
        let viewer_ids = iter::repeat_with(ParticipantId::new).take(PRESET_COLORS.len());
        let mut id_to_expected_color = HashMap::new();

        for (i, id) in viewer_ids.enumerate() {
            // Add a new viewer.
            viewers.push(Viewer {
                info: ParticipantInfo {
                    id: id.clone(),
                    ..Default::default()
                },
                role: Role::Reader,
                is_present: true,
            });
            let sharer_clone = sharer.clone();
            let viewers_clone = viewers.clone();
            presence_manager
                .update(&mut app, |presence_manager, ctx| {
                    presence_manager.update_participants(
                        ParticipantList {
                            sharer: sharer_clone,
                            viewers: viewers_clone,
                            present_viewers: Default::default(),
                            absent_viewers: Default::default(),
                            guests: Default::default(),
                            pending_guests: Default::default(),
                        },
                        ctx,
                    );
                    let spawned_future = presence_manager
                        .load_participants_imgs_future_handle
                        .as_ref()
                        .expect("should have future handle");
                    ctx.await_spawned_future(spawned_future.future_id())
                })
                .await;

            // Expect the new viewer to take the next preset color, while continuing to expect old viewers to keep their colors.
            id_to_expected_color.insert(id, PRESET_COLORS[i]);
            presence_manager.read(&app, |presence_manager, _ctx| {
                let viewers = presence_manager.get_present_viewers().collect_vec();
                assert_eq!(viewers.len(), i + 1);
                for viewer in presence_manager.get_present_viewers() {
                    let expected_color = *id_to_expected_color
                        .get(&viewer.info.id)
                        .expect("should have expected viewer ids only");
                    assert_eq!(viewer.color, expected_color);
                    assert!(matches!(viewer.role, Some(Role::Reader)));
                }
            });
        }

        // Set the first viewer as no longer present, and add a new participant.
        viewers.get_mut(0).unwrap().is_present = false;
        assert!(!viewers.first().unwrap().is_present);
        let old_participant_id = viewers.first().unwrap().info.id.clone();
        let new_id = ParticipantId::new();
        viewers.push(Viewer {
            info: ParticipantInfo {
                id: new_id.clone(),
                ..Default::default()
            },
            role: Role::Reader,
            is_present: true,
        });
        presence_manager
            .update(&mut app, |presence_manager, ctx| {
                presence_manager.update_participants(
                    ParticipantList {
                        sharer,
                        viewers,
                        present_viewers: Default::default(),
                        absent_viewers: Default::default(),
                        guests: Default::default(),
                        pending_guests: Default::default(),
                    },
                    ctx,
                );
                let spawned_future = presence_manager
                    .load_participants_imgs_future_handle
                    .as_ref()
                    .expect("should have future handle");
                ctx.await_spawned_future(spawned_future.future_id())
            })
            .await;

        // The color previously taken by the first viewer should be reused for the new participant, while other participants keep their existing colors.
        let old_participant_color = id_to_expected_color
            .remove(&old_participant_id)
            .expect("old participant exists");
        id_to_expected_color.insert(new_id, old_participant_color);
        presence_manager.read(&app, |presence_manager, _ctx| {
            let viewers = presence_manager.get_present_viewers().collect_vec();
            assert_eq!(viewers.len(), PRESET_COLORS.len());
            for viewer in viewers {
                assert_eq!(
                    viewer.color,
                    *id_to_expected_color
                        .get(&viewer.info.id)
                        .expect("should have expected viewer ids only")
                );
                assert!(matches!(viewer.role, Some(Role::Reader)));
            }
        });
    });
}

#[test]
fn test_dont_include_self_in_viewers() {
    App::test((), |mut app| async move {
        let self_id = ParticipantId::new();
        let self_firebase_uid = UserUid::new("mock_firebase_uid");

        let sharer = Sharer {
            ..Default::default()
        };
        let viewers = vec![
            Viewer {
                info: ParticipantInfo {
                    id: self_id.clone(),
                    ..Default::default()
                },
                role: Role::Reader,
                is_present: true,
            },
            Viewer {
                info: ParticipantInfo {
                    ..Default::default()
                },
                role: Role::Reader,
                is_present: true,
            },
            Viewer {
                info: ParticipantInfo {
                    ..Default::default()
                },
                role: Role::Reader,
                is_present: true,
            },
            Viewer {
                info: ParticipantInfo {
                    ..Default::default()
                },
                role: Role::Reader,
                is_present: true,
            },
        ];
        let participant_list = ParticipantList {
            sharer,
            viewers,
            present_viewers: Default::default(),
            absent_viewers: Default::default(),
            guests: Default::default(),
            pending_guests: Default::default(),
        };

        let presence_manager = app.add_model(|ctx| {
            PresenceManager::new_for_viewer(
                self_id.clone(),
                self_firebase_uid,
                participant_list.clone(),
                ctx,
            )
        });

        // Ensure participants are loaded before continuing.
        presence_manager
            .update(&mut app, |presence_manager, ctx| {
                let spawned_future = presence_manager
                    .load_participants_imgs_future_handle
                    .as_ref()
                    .expect("should have future handle");
                ctx.await_spawned_future(spawned_future.future_id())
            })
            .await;

        presence_manager.read(&app, |presence_manager, _ctx| {
            let mut participant_colors = HashSet::new();
            let sharer = presence_manager.get_sharer().expect("should have sharer");
            participant_colors.insert(sharer.color);

            // The viewers returned by presence manager should not include ourselves.
            let viewers = presence_manager.get_present_viewers().collect_vec();
            assert_eq!(viewers.len(), 3);
            for viewer in viewers {
                assert_ne!(viewer.info.id, self_id);
                participant_colors.insert(viewer.color);
            }

            // The sharer and 3 other viewers should all use colors from the preset colors.
            let preset_colors = HashSet::from_iter(PRESET_COLORS[..4].iter().copied());
            assert!(participant_colors.eq(&preset_colors));
        });
    });
}

#[test]
fn query_attribution_profile_retains_absent_viewers_without_using_the_sharer() {
    let sharer_id = ParticipantId::new();
    let mut manager = PresenceManager::new_for_sharer(sharer_id.clone(), UserUid::new("host"));
    let viewer_id = ParticipantId::new();
    let info = ParticipantInfo {
        id: viewer_id.clone(),
        profile_data: ProfileData {
            firebase_uid: "viewer".into(),
            email: Some("viewer@example.com".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    manager.present_viewers.insert(
        viewer_id.clone(),
        super::Participant {
            info: info.clone(),
            color: PRESET_COLORS[0],
            role: Some(Role::Executor),
        },
    );
    assert_eq!(
        manager
            .participant_profile(&viewer_id)
            .unwrap()
            .firebase_uid,
        "viewer"
    );
    manager.present_viewers.remove(&viewer_id);
    manager.absent_viewers.insert(
        viewer_id.clone(),
        super::AbsentViewer {
            participant_info: info,
        },
    );
    let profile = manager.participant_profile(&viewer_id).unwrap();
    assert_eq!(profile.firebase_uid, "viewer");
    assert_eq!(profile.email.as_deref(), Some("viewer@example.com"));
    assert!(manager.participant_profile(&sharer_id).is_none());
    assert!(manager.participant_profile(&ParticipantId::new()).is_none());
}
