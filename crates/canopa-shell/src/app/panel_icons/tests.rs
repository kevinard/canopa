use super::*;

#[test]
fn prepared_icons_contain_pixels_and_need_no_source_file_at_render_time() {
    let temp = tempfile::tempdir().unwrap();
    let svg = temp.path().join("icon.svg");
    std::fs::write(&svg, r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20"><rect width="40" height="20" fill="#ff0000" opacity="0.5"/></svg>"##).unwrap();
    let png = temp.path().join("icon.png");
    image::RgbaImage::from_pixel(80, 40, image::Rgba([0, 255, 0, 128]))
        .save(&png)
        .unwrap();
    for (path, expected) in [(&svg, [255, 0, 0]), (&png, [0, 255, 0])] {
        let handle = prepare::from_path(path).expect("prepared image");
        std::fs::remove_file(path).unwrap();
        let Handle::Rgba {
            width,
            height,
            pixels,
            ..
        } = handle
        else {
            panic!("renderer must receive decoded pixels, never paths or encoded bytes");
        };
        assert_eq!((width, height), (72, 36));
        assert_eq!(&pixels[..3], &expected);
        assert!((127..=128).contains(&pixels[3]));
    }
    let broken = temp.path().join("broken.png");
    std::fs::write(&broken, b"broken").unwrap();
    assert!(prepare::from_path(&broken).is_none());
}

#[tokio::test]
async fn lookup_runs_off_the_caller_thread_and_missing_icons_complete() {
    crate::icon_theme::take_lookup_count();
    let loaded = load(vec!["../invalid-icon".to_owned()]).await;
    assert_eq!(crate::icon_theme::take_lookup_count(), 0);
    assert_eq!(loaded, vec![("../invalid-icon".to_owned(), None)]);
}

#[test]
fn icon_work_is_bounded_deduplicated_and_retargets_after_scrolling() {
    let mut menu = PanelMenuRenderState {
        entries: (0..100)
            .map(|i| super::super::panel_menu_session::PanelMenuEntry {
                label: format!("App {i}"),
                icon: Some(format!("icon-{}", i / 2)),
                path: vec![],
                target: super::super::panel_menu_session::PanelMenuEntryTarget::InstalledApp(
                    format!("app-{i}"),
                ),
                is_submenu: false,
            })
            .collect(),
        show_icons: true,
        prompt: None,
        search_query: String::new(),
        focused_index: None,
        can_go_back: false,
        scroll_y: 0.0,
        viewport_height: 300.0,
    };
    let mut icons = PanelIcons::default();
    let names = icons.request(&menu, 45.0);
    assert_eq!(names, ["icon-0", "icon-1", "icon-2", "icon-3"]);
    menu.scroll_y = 60.0 * super::super::panel::MENU_ROW_EXTENT;
    assert!(
        icons.request(&menu, 45.0).is_empty(),
        "only one batch in flight"
    );
    icons.complete(names.into_iter().map(|name| (name, None)).collect());
    assert_eq!(
        icons.request(&menu, 45.0),
        ["icon-30", "icon-31", "icon-32", "icon-33"]
    );
    icons.complete(vec![]);
    menu.scroll_y = 0.0;
    assert_eq!(
        icons.request(&menu, 45.0),
        ["icon-4", "icon-5", "icon-6", "icon-7"]
    );
    icons.complete(vec![]);
    menu.show_icons = false;
    assert!(icons.request(&menu, 45.0).is_empty());

    for i in 0..CACHE_SIZE + 10 {
        icons.complete(vec![(format!("bounded-{i}"), None)]);
    }
    assert_eq!(icons.ready.len(), CACHE_SIZE);
    assert_eq!(icons.order.len(), CACHE_SIZE);
}
