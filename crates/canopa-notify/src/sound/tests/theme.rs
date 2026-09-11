use super::*;

fn named_playback(name: &str) -> NotificationSoundPlayback {
    NotificationSoundPlayback {
        requested_sound: NotificationSound::name(name),
        ..file_playback(Path::new("unused"))
    }
}

fn assert_silent_lookup_finishes(
    presenter: &RodioSoundPresenter,
    playback: NotificationSoundPlayback,
    probe: &Path,
) {
    for _ in 0..4 {
        presenter.play_notification_sound(playback.clone()).unwrap();
    }
    // Never consume mixer output. Four audible probe jobs can be admitted only
    // if every preceding lookup finished without retaining any audio source.
    for _ in 0..4 {
        wait_until(|| {
            presenter
                .play_notification_sound(file_playback(probe))
                .is_ok()
        });
    }
}

fn install_theme(root: &Path, name: &str, directories: &[(&str, &str)], parents: &str) -> PathBuf {
    let base = root.join("sounds").join(name);
    std::fs::create_dir_all(&base).unwrap();
    let mut index = format!(
        "[Sound Theme]\nName=Test\nComment=Test\nDirectories={}\nInherits={parents}\n",
        directories
            .iter()
            .map(|(path, _)| *path)
            .collect::<Vec<_>>()
            .join(",")
    );
    for (path, profile) in directories {
        std::fs::create_dir_all(base.join(path)).unwrap();
        index.push_str(&format!("[{path}]\nOutputProfile={profile}\n"));
    }
    std::fs::write(base.join("index.theme"), index).unwrap();
    base
}

fn assert_sound_samples(
    presenter: &RodioSoundPresenter,
    output: &mut rodio::mixer::MixerSource,
    name: &str,
    expected: usize,
) {
    wait_until(|| {
        presenter
            .play_notification_sound(named_playback(name))
            .is_ok()
    });
    wait_until(|| output.next().is_some());
    assert_eq!(1 + output.by_ref().count(), expected);
}

#[test]
fn locale_fallback_preserves_modifier_territory_language_c_and_unlocalized_order() {
    let temp = tempfile::tempdir().unwrap();
    let theme = install_theme(temp.path(), "freedesktop", &[("stereo", "stereo")], "");
    let variants = ["fr_CA@formal", "fr_CA", "fr", "C", ""];
    for (offset, locale) in variants.iter().enumerate() {
        let directory = theme.join("stereo").join(locale);
        std::fs::create_dir_all(&directory).unwrap();
        wave_file(&directory.join("event.wav"), 8 * (offset as u32 + 1));
    }
    let (presenter, mut output) = offline_presenter_with_resolver(
        SoundThemeResolver::new(vec![temp.path().to_path_buf()], "fr_CA.UTF-8@formal"),
        1,
        8_000,
    );
    for (offset, locale) in variants.iter().enumerate() {
        assert_sound_samples(&presenter, &mut output, "event", 8 * (offset + 1));
        // Removing a fixture must affect the next request without restarting.
        std::fs::remove_file(theme.join("stereo").join(locale).join("event.wav")).unwrap();
    }
}

#[test]
fn first_index_and_ordered_roots_control_themed_overrides_and_refresh() {
    let temp = tempfile::tempdir().unwrap();
    let user = temp.path().join("user");
    let system = temp.path().join("system");
    let custom = install_theme(
        &user,
        "custom",
        &[("alerts", "stereo")],
        "freedesktop,parent",
    );
    install_theme(&system, "custom", &[("different", "stereo")], "wrong");
    let parent = install_theme(&system, "parent", &[("stereo", "stereo")], "");
    let wrong = install_theme(&system, "wrong", &[("stereo", "stereo")], "");
    let fallback = install_theme(&system, "freedesktop", &[("stereo", "stereo")], "");
    std::fs::create_dir_all(system.join("sounds/custom/alerts")).unwrap();
    wave_file(&custom.join("alerts/event.wav"), 8);
    wave_file(&system.join("sounds/custom/alerts/event.wav"), 16);
    wave_file(&system.join("sounds/custom/different/event.wav"), 24);
    wave_file(&parent.join("stereo/inherited.wav"), 32);
    wave_file(&fallback.join("stereo/inherited.wav"), 40);
    wave_file(&wrong.join("stereo/inherited.wav"), 48);
    let (presenter, mut output) = offline_presenter_with_dirs(vec![user.clone(), system]);
    presenter
        .apply_sound_settings(NotificationSoundSettings {
            theme: "custom".into(),
            ..Default::default()
        })
        .unwrap();
    assert_sound_samples(&presenter, &mut output, "event", 8);
    std::fs::remove_file(custom.join("alerts/event.wav")).unwrap();
    assert_sound_samples(&presenter, &mut output, "event", 16);
    assert_sound_samples(&presenter, &mut output, "inherited", 32);
    // The selected index is reread, not cached forever or merged with the lower root.
    install_theme(&user, "custom", &[("different", "stereo")], "");
    assert_sound_samples(&presenter, &mut output, "event", 24);
}

#[test]
fn inheritance_cycles_terminate_and_profileless_then_unthemed_fallback_work() {
    let temp = tempfile::tempdir().unwrap();
    let custom = install_theme(
        temp.path(),
        "custom",
        &[("alerts", "stereo"), (".", "")],
        "parent",
    );
    install_theme(temp.path(), "parent", &[], "custom");
    let fallback = install_theme(temp.path(), "freedesktop", &[("stereo", "stereo")], "");
    std::os::unix::fs::symlink(&custom, custom.join("unlisted-cycle")).unwrap();
    wave_file(&custom.join("profileless.wav"), 8);
    wave_file(&fallback.join("stereo/fallback.wav"), 16);
    wave_file(&temp.path().join("sounds/unthemed.wav"), 24);
    let (presenter, mut output) = offline_presenter_with_dirs(vec![temp.path().to_path_buf()]);
    presenter
        .apply_sound_settings(NotificationSoundSettings {
            theme: "custom".into(),
            ..Default::default()
        })
        .unwrap();
    assert_sound_samples(&presenter, &mut output, "profileless", 8);
    assert_sound_samples(&presenter, &mut output, "fallback", 16);
    assert_sound_samples(&presenter, &mut output, "unthemed", 24);
}

#[test]
fn disabled_generic_localized_and_default_sounds_all_stop_fallback() {
    for (requested, disabled, default) in [
        (Some("event-new-mail"), "event", "message-new-instant"),
        (Some("event"), "C/event", "message-new-instant"),
        (None, "message-new-instant", "message-new-instant"),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let custom = install_theme(temp.path(), "custom", &[(".", "")], "freedesktop");
        let fallback = install_theme(temp.path(), "freedesktop", &[("stereo", "stereo")], "");
        std::fs::create_dir_all(custom.join("C")).unwrap();
        std::fs::write(custom.join(format!("{disabled}.disabled")), "").unwrap();
        let probe = fallback.join("stereo/message-new-instant.wav");
        wave_file(&probe, 8_000);
        wave_file(
            &fallback.join(format!("stereo/{}.wav", requested.unwrap_or(default))),
            8_000,
        );
        let (presenter, _output) = offline_presenter_with_dirs(vec![temp.path().to_path_buf()]);
        presenter
            .apply_sound_settings(NotificationSoundSettings {
                theme: "custom".into(),
                ..Default::default()
            })
            .unwrap();
        let playback = NotificationSoundPlayback {
            requested_sound: requested.and_then(NotificationSound::name),
            ..named_playback(default)
        };
        assert_silent_lookup_finishes(&presenter, playback, &probe);
    }
}

#[test]
fn malformed_oversized_and_special_first_indexes_stop_without_default_or_lower_index() {
    for kind in [
        "oversized",
        "invalid-utf8",
        "parent-path",
        "directory-path",
        "missing-section",
        "duplicate-key",
        "fifo",
        "directory",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let user = temp.path().join("user");
        let system = temp.path().join("system");
        let custom = install_theme(&user, "custom", &[(".", "")], "");
        let index = custom.join("index.theme");
        let lower = install_theme(&system, "custom", &[("stereo", "stereo")], "");
        let fallback = install_theme(&system, "freedesktop", &[("stereo", "stereo")], "");
        wave_file(&lower.join("stereo/event.wav"), 8_000);
        let probe = fallback.join("stereo/message-new-instant.wav");
        wave_file(&probe, 8_000);
        match kind {
            "oversized" => File::options()
                .write(true)
                .open(&index)
                .unwrap()
                .set_len(65_537)
                .unwrap(),
            "invalid-utf8" => std::fs::write(&index, [0xff]).unwrap(),
            "parent-path" => {
                std::fs::write(&index, "[Sound Theme]\nDirectories=\nInherits=../escape\n").unwrap()
            }
            "directory-path" => std::fs::write(
                &index,
                "[Sound Theme]\nDirectories=../escape\n[../escape]\n",
            )
            .unwrap(),
            "missing-section" => {
                std::fs::write(&index, "[Sound Theme]\nDirectories=absent\n").unwrap()
            }
            "duplicate-key" => {
                std::fs::write(&index, "[Sound Theme]\nDirectories=\nDirectories=stereo\n").unwrap()
            }
            "fifo" => {
                std::fs::remove_file(&index).unwrap();
                rustix::fs::mkfifoat(
                    rustix::fs::CWD,
                    &index,
                    rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                )
                .unwrap();
            }
            "directory" => {
                std::fs::remove_file(&index).unwrap();
                std::fs::create_dir(&index).unwrap();
            }
            _ => unreachable!(),
        }
        let (presenter, _output) = offline_presenter_with_dirs(vec![user, system]);
        presenter
            .apply_sound_settings(NotificationSoundSettings {
                theme: "custom".into(),
                ..Default::default()
            })
            .unwrap();
        assert_silent_lookup_finishes(&presenter, named_playback("event"), &probe);
    }
}

#[test]
fn theme_count_limit_allows_thirty_two_and_stops_before_thirty_three() {
    for theme_count in [32, 33] {
        let temp = tempfile::tempdir().unwrap();
        for index in 0..theme_count {
            let parent = if index + 1 < theme_count {
                format!("theme{}", index + 1)
            } else {
                String::new()
            };
            let theme = install_theme(
                temp.path(),
                &format!("theme{index}"),
                &[("stereo", "stereo")],
                &parent,
            );
            if index + 1 == theme_count {
                wave_file(&theme.join("stereo/event.wav"), 8);
            }
        }
        let fallback = install_theme(temp.path(), "freedesktop", &[("stereo", "stereo")], "");
        let probe = fallback.join("stereo/message-new-instant.wav");
        wave_file(&probe, 8_000);
        let (presenter, mut output) = offline_presenter_with_dirs(vec![temp.path().to_path_buf()]);
        presenter
            .apply_sound_settings(NotificationSoundSettings {
                theme: "theme0".into(),
                ..Default::default()
            })
            .unwrap();
        if theme_count == 32 {
            assert_sound_samples(&presenter, &mut output, "event", 8);
        } else {
            assert_silent_lookup_finishes(&presenter, named_playback("event"), &probe);
        }
    }
}

#[test]
fn lookup_work_and_root_limits_skip_sound_without_leaking_admission() {
    let temp = tempfile::tempdir().unwrap();
    let roots = (0..32)
        .map(|index| temp.path().join(format!("root{index}")))
        .collect::<Vec<_>>();
    let names = (0..64)
        .map(|index| format!("dir{index}"))
        .collect::<Vec<_>>();
    let directories = names
        .iter()
        .map(|name| (name.as_str(), "stereo"))
        .collect::<Vec<_>>();
    let custom = install_theme(&roots[0], "custom", &directories, "");
    wave_file(&custom.join("dir63/event-a-b-c-d-e-f-g.wav"), 8_000);
    let fallback = install_theme(&roots[0], "freedesktop", &[("stereo", "stereo")], "");
    let probe = fallback.join("stereo/message-new-instant.wav");
    wave_file(&probe, 8_000);
    let (presenter, _output) = offline_presenter_with_dirs(roots.clone());
    presenter
        .apply_sound_settings(NotificationSoundSettings {
            theme: "custom".into(),
            ..Default::default()
        })
        .unwrap();
    assert_silent_lookup_finishes(&presenter, named_playback("event-a-b-c-d-e-f-g"), &probe);

    let (presenter, mut output) = offline_presenter_with_dirs(roots.clone());
    assert_sound_samples(&presenter, &mut output, "message-new-instant", 8_000);
    let mut too_many = roots;
    too_many.push(temp.path().join("root32"));
    let (presenter, _output) = offline_presenter_with_dirs(too_many);
    assert_silent_lookup_finishes(&presenter, named_playback("message-new-instant"), &probe);
}

#[test]
fn exact_index_and_directory_limits_remain_usable() {
    let temp = tempfile::tempdir().unwrap();
    let names = (0..64)
        .map(|index| format!("dir{index}"))
        .collect::<Vec<_>>();
    let directories = names
        .iter()
        .map(|name| (name.as_str(), "stereo"))
        .collect::<Vec<_>>();
    let theme = install_theme(temp.path(), "freedesktop", &directories, "");
    let index = theme.join("index.theme");
    let mut contents = std::fs::read_to_string(&index).unwrap();
    contents.push_str(&"#".repeat(65_536 - contents.len()));
    std::fs::write(&index, contents).unwrap();
    wave_file(&theme.join("dir63/event.wav"), 8);
    let (presenter, mut output) = offline_presenter_with_dirs(vec![temp.path().to_path_buf()]);
    assert_sound_samples(&presenter, &mut output, "event", 8);
}

#[test]
fn themed_extension_order_prefers_standard_formats_before_local_extensions() {
    let temp = tempfile::tempdir().unwrap();
    let theme = install_theme(temp.path(), "freedesktop", &[("stereo", "stereo")], "");
    let extensions = ["oga", "ogg", "wav", "flac", "mp3"];
    for (index, extension) in extensions.iter().enumerate() {
        // WAV payloads isolate filename precedence from decoder-format testing.
        wave_file(
            &theme.join(format!("stereo/event.{extension}")),
            8 * (index as u32 + 1),
        );
    }
    let (presenter, mut output) = offline_presenter_with_dirs(vec![temp.path().to_path_buf()]);
    for (index, extension) in extensions.iter().enumerate() {
        assert_sound_samples(&presenter, &mut output, "event", 8 * (index + 1));
        std::fs::remove_file(theme.join(format!("stereo/event.{extension}"))).unwrap();
    }
}

#[test]
fn excessive_index_lists_are_rejected_without_audio_or_default_fallback() {
    for excess_directories in [true, false] {
        let temp = tempfile::tempdir().unwrap();
        let names = (0..65)
            .map(|index| format!("dir{index}"))
            .collect::<Vec<_>>();
        let directories = names
            .iter()
            .map(|name| (name.as_str(), "stereo"))
            .collect::<Vec<_>>();
        let parents = (0..33)
            .map(|index| format!("parent{index}"))
            .collect::<Vec<_>>()
            .join(",");
        let custom = if excess_directories {
            install_theme(temp.path(), "custom", &directories, "")
        } else {
            install_theme(temp.path(), "custom", &[("dir0", "stereo")], &parents)
        };
        wave_file(&custom.join("dir0/event.wav"), 8_000);
        let fallback = install_theme(temp.path(), "freedesktop", &[("stereo", "stereo")], "");
        let probe = fallback.join("stereo/message-new-instant.wav");
        wave_file(&probe, 8_000);
        let (presenter, _output) = offline_presenter_with_dirs(vec![temp.path().to_path_buf()]);
        presenter
            .apply_sound_settings(NotificationSoundSettings {
                theme: "custom".into(),
                ..Default::default()
            })
            .unwrap();
        assert_silent_lookup_finishes(&presenter, named_playback("event"), &probe);
    }
}

#[test]
fn disabled_themed_sound_never_plays_same_name_parent_or_default() {
    let temp = tempfile::tempdir().unwrap();
    let theme = temp.path().join("sounds/custom/stereo");
    let fallback = temp.path().join("sounds/freedesktop/stereo");
    std::fs::create_dir_all(&theme).unwrap();
    std::fs::create_dir_all(&fallback).unwrap();
    std::fs::write(theme.parent().unwrap().join("index.theme"),
        "[Sound Theme]\nName=Custom\nComment=Test\nDirectories=stereo\nInherits=freedesktop\n[stereo]\nOutputProfile=stereo\n").unwrap();
    std::fs::write(theme.join("message.disabled"), "").unwrap();
    wave_file(&theme.join("message.wav"), 8_000);
    wave_file(&fallback.join("message.wav"), 8_000);
    wave_file(&fallback.join("message-new-instant.wav"), 8_000);
    let (presenter, _output) = offline_presenter_with_dirs(vec![temp.path().to_path_buf()]);
    presenter
        .apply_sound_settings(NotificationSoundSettings {
            theme: "custom".into(),
            ..Default::default()
        })
        .unwrap();

    assert_silent_lookup_finishes(
        &presenter,
        named_playback("message"),
        &theme.join("message.wav"),
    );
}

#[test]
fn themed_sound_uses_only_declared_directories_matching_the_output_profile() {
    let temp = tempfile::tempdir().unwrap();
    let theme = temp.path().join("sounds/freedesktop");
    for dir in ["stereo", "surround", "alerts"] {
        std::fs::create_dir_all(theme.join(dir)).unwrap();
    }
    std::fs::write(theme.join("index.theme"),
        "[Sound Theme]\nName=Test\nComment=Test\nDirectories=surround,alerts\n[surround]\nOutputProfile=5.1\n[alerts]\nOutputProfile=stereo\n").unwrap();
    wave_file(&theme.join("stereo/message.wav"), 8);
    wave_file(&theme.join("surround/message.wav"), 16);
    wave_file(&theme.join("alerts/message.wav"), 24);
    let (presenter, mut output) = offline_presenter_with_dirs(vec![temp.path().to_path_buf()]);

    presenter
        .play_notification_sound(named_playback("message"))
        .unwrap();
    wait_until(|| output.next().is_some());
    assert_eq!(1 + output.by_ref().count(), 24);
}

#[test]
fn generic_event_in_selected_theme_overrides_exact_event_in_parent() {
    let temp = tempfile::tempdir().unwrap();
    let custom = temp.path().join("sounds/custom");
    let parent = temp.path().join("sounds/parent");
    for path in [&custom, &parent] {
        std::fs::create_dir_all(path.join("stereo")).unwrap();
    }
    std::fs::write(
        custom.join("index.theme"),
        "[Sound Theme]\nDirectories=stereo\nInherits=parent\n[stereo]\nOutputProfile=stereo\n",
    )
    .unwrap();
    wave_file(&custom.join("stereo/message.wav"), 24);
    wave_file(&parent.join("stereo/message-new-email.wav"), 8);
    let (presenter, mut output) = offline_presenter_with_dirs(vec![temp.path().to_path_buf()]);
    presenter
        .apply_sound_settings(NotificationSoundSettings {
            theme: "custom".into(),
            ..Default::default()
        })
        .unwrap();
    presenter
        .play_notification_sound(named_playback("message-new-email"))
        .unwrap();
    wait_until(|| output.next().is_some());
    assert_eq!(1 + output.by_ref().count(), 24);
}

#[test]
fn localized_sound_precedes_unlocalized_sound_in_the_same_directory() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("sounds/freedesktop/stereo");
    std::fs::create_dir_all(directory.join("C")).unwrap();
    wave_file(&directory.join("message.wav"), 8);
    wave_file(&directory.join("C/message.wav"), 24);
    let (presenter, mut output) = offline_presenter_with_dirs(vec![temp.path().to_path_buf()]);
    presenter
        .play_notification_sound(named_playback("message"))
        .unwrap();
    wait_until(|| output.next().is_some());
    assert_eq!(1 + output.by_ref().count(), 24);
}
