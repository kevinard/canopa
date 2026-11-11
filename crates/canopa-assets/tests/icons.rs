use canopa_assets::icons::{IconFormats, IconLookup, LookupOptions};
use std::path::{Path, PathBuf};

fn write(root: &Path, relative: &str, contents: &str) -> PathBuf {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, contents).unwrap();
    path
}

fn options(formats: IconFormats) -> LookupOptions<'static> {
    LookupOptions {
        theme: "hicolor",
        scale: 1,
        formats,
        allow_unindexed_hicolor: false,
    }
}

#[test]
fn formats_share_indexed_lookup_without_leaking_svg_to_raster_consumers() {
    let temp = tempfile::tempdir().unwrap();
    let roots = vec![temp.path().to_owned()];
    write(
        temp.path(),
        "hicolor/index.theme",
        "[Icon Theme]\nDirectories=scalable/apps\n[scalable/apps]\nSize=22\nType=Scalable\nMinSize=16\nMaxSize=128\n",
    );
    let svg = write(temp.path(), "hicolor/scalable/apps/app.svg", "<svg/>");
    let jpeg = write(temp.path(), "hicolor/scalable/apps/app.jpeg", "jpeg");
    let names = ["app".into()];
    let mut raster = IconLookup::new(&roots, options(IconFormats::PngJpeg), &|| false);
    let mut shell = IconLookup::new(&roots, options(IconFormats::PngSvgXpm), &|| false);
    assert_eq!(raster.resolve(&names, 22), Some(jpeg));
    assert_eq!(shell.resolve(&names, 22), Some(svg));
}

#[test]
fn indexless_hicolor_compatibility_preserves_svg_grid_and_unthemed_fallbacks() {
    let temp = tempfile::tempdir().unwrap();
    let roots = vec![temp.path().to_owned()];
    let svg = write(temp.path(), "hicolor/scalable/apps/app.svg", "<svg/>");
    let grid = write(temp.path(), "hicolor/22x22/apps/app.png", "png");
    let fallback = write(temp.path(), "fallback.svg", "<svg/>");
    let make = || {
        IconLookup::new(
            &roots,
            LookupOptions {
                allow_unindexed_hicolor: true,
                ..options(IconFormats::PngSvgXpm)
            },
            &|| false,
        )
    };
    assert_eq!(make().resolve(&["app".into()], 22), Some(svg.clone()));
    std::fs::remove_file(svg).unwrap();
    assert_eq!(make().resolve(&["app".into()], 22), Some(grid));
    assert_eq!(make().resolve(&["fallback".into()], 22), Some(fallback));
    assert!(
        IconLookup::new(&roots, options(IconFormats::PngJpeg), &|| false)
            .resolve(&["app".into()], 22)
            .is_none()
    );
}

#[test]
fn selected_theme_scale_and_first_index_take_precedence_over_parents_and_roots() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    let roots = vec![first.clone(), second.clone()];
    write(
        &first,
        "Selected/index.theme",
        "[Icon Theme]\nInherits=Parent\nDirectories=normal,scaled\n[normal]\nSize=44\nType=Fixed\n[scaled]\nSize=22\nScale=2\nType=Fixed\n",
    );
    write(
        &second,
        "Selected/index.theme",
        "invalid lower-priority index",
    );
    write(&first, "Selected/normal/app.svg", "<svg/>");
    let expected = write(&second, "Selected/scaled/app.svg", "<svg/>");
    write(
        &first,
        "Parent/index.theme",
        "[Icon Theme]\nDirectories=apps\n[apps]\nSize=22\nScale=2\nType=Fixed\n",
    );
    let parent = write(&first, "Parent/apps/parent-only.svg", "<svg/>");
    let mut lookup = IconLookup::new(
        &roots,
        LookupOptions {
            theme: "Selected",
            scale: 2,
            ..options(IconFormats::PngSvgXpm)
        },
        &|| false,
    );
    assert_eq!(lookup.resolve(&["app".into()], 22), Some(expected));
    assert_eq!(lookup.resolve(&["parent-only".into()], 22), Some(parent));
}

#[test]
fn malformed_indexes_cancellation_and_excess_roots_never_select_lower_priority_assets() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_owned();
    let roots = vec![root.clone()];
    write(&root, "hicolor/index.theme", "invalid");
    write(&root, "hicolor/scalable/apps/app.svg", "<svg/>");
    write(&root, "app.svg", "<svg/>");
    let names = ["app".into()];
    let mut lookup = IconLookup::new(
        &roots,
        LookupOptions {
            allow_unindexed_hicolor: true,
            ..options(IconFormats::PngSvgXpm)
        },
        &|| false,
    );
    assert!(lookup.resolve(&names, 22).is_none());
    assert!(
        IconLookup::new(&roots, options(IconFormats::PngSvgXpm), &|| true)
            .resolve(&names, 22)
            .is_none()
    );
    let excess = vec![root; 33];
    assert!(
        IconLookup::new(&excess, options(IconFormats::PngSvgXpm), &|| false)
            .resolve(&names, 22)
            .is_none()
    );
}

fn theme_index(root: &Path, theme: &str, inherits: &str, directories: &[&str]) {
    let mut index = format!(
        "[Icon Theme]\nInherits={inherits}\nDirectories={}\n",
        directories.join(",")
    );
    for directory in directories {
        index.push_str(&format!(
            "[{directory}]\nSize=48\nType=Scalable\nMinSize=8\nMaxSize=512\n"
        ));
    }
    write(root, &format!("{theme}/index.theme"), &index);
}

#[test]
fn every_name_is_tried_in_the_selected_theme_and_its_parents_before_hicolor() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let roots = vec![root.to_owned()];
    theme_index(root, "Selected", "Middle", &["apps"]);
    theme_index(root, "Middle", "Base,hicolor", &["apps"]);
    theme_index(root, "Base", "", &["apps"]);
    theme_index(root, "hicolor", "", &["apps"]);
    write(root, "hicolor/apps/first.svg", "<svg/>");
    let base = write(root, "Base/apps/second.svg", "<svg/>");
    let names = ["first".into(), "second".into()];
    let mut lookup = IconLookup::new(
        &roots,
        LookupOptions {
            theme: "Selected",
            ..options(IconFormats::PngSvgXpm)
        },
        &|| false,
    );
    assert_eq!(lookup.resolve(&names, 48), Some(base));
    assert_eq!(
        lookup.resolve(&["first".into()], 48),
        Some(root.join("hicolor/apps/first.svg"))
    );
}

#[test]
fn generic_fallbacks_follow_symbolic_then_regular_dash_truncation() {
    use canopa_assets::icons::candidate_names;
    assert_eq!(
        candidate_names(&["mail-unread-symbolic".into(), "x".into()], true),
        [
            "mail-unread-symbolic",
            "x",
            "mail-symbolic",
            "mail-unread",
            "mail"
        ]
    );
    assert_eq!(
        candidate_names(&["input-mouse-usb".into(), "../bad".into()], true),
        ["input-mouse-usb", "input-mouse", "input"]
    );
    assert_eq!(candidate_names(&["a-b".into()], false), ["a-b"]);

    let temp = tempfile::tempdir().unwrap();
    let roots = vec![temp.path().to_owned()];
    theme_index(temp.path(), "hicolor", "", &["status"]);
    let generic = write(temp.path(), "hicolor/status/dialog-symbolic.svg", "<svg/>");
    let names = ["dialog-warning-symbolic".into()];
    let lookup = || IconLookup::new(&roots, options(IconFormats::PngSvgXpm), &|| false);
    assert_eq!(lookup().resolve(&names, 16), None);
    assert_eq!(
        lookup().with_generic_fallback(true).resolve(&names, 16),
        Some(generic)
    );
}

#[test]
fn icon_sources_accept_absolute_paths_file_urls_and_strip_legacy_extensions() {
    use canopa_assets::icons::IconSource;
    assert_eq!(
        IconSource::parse(" /opt/app/icon.png "),
        Some(IconSource::Path("/opt/app/icon.png".into()))
    );
    assert_eq!(
        IconSource::parse("file:///tmp/My%20Icon.svg"),
        Some(IconSource::Path("/tmp/My Icon.svg".into()))
    );
    assert_eq!(
        IconSource::parse("FILE://localhost/tmp/a"),
        Some(IconSource::Path("/tmp/a".into()))
    );
    assert_eq!(
        IconSource::parse("steam_icon_42.png"),
        Some(IconSource::Name("steam_icon_42".into()))
    );
    assert_eq!(
        IconSource::parse("org.gnome.Nautilus"),
        Some(IconSource::Name("org.gnome.Nautilus".into()))
    );
    for rejected in [
        "",
        "relative/icon.png",
        "file://host/tmp/a",
        "https://example.com/a.png",
        "/tmp/../etc/a.png",
        "file:///tmp/%00",
    ] {
        assert_eq!(IconSource::parse(rejected), None, "{rejected}");
    }
}

#[test]
fn resolver_follows_the_desktop_theme_and_refreshes_when_settings_change() {
    use canopa_assets::icons::{IconQuery, IconResolver, IconSource};
    let temp = tempfile::tempdir().unwrap();
    let icons = temp.path().join("icons");
    theme_index(&icons, "Papirus", "", &["apps"]);
    theme_index(&icons, "Breeze", "", &["apps"]);
    theme_index(&icons, "hicolor", "", &["apps"]);
    let papirus = write(&icons, "Papirus/apps/app.svg", "<svg/>");
    let breeze = write(&icons, "Breeze/apps/app.svg", "<svg/>");
    let hicolor = write(&icons, "hicolor/apps/app.svg", "<svg/>");
    let settings = write(
        temp.path(),
        "config/gtk-3.0/settings.ini",
        "[Settings]\ngtk-icon-theme-name=Papirus\n",
    );
    let resolver = IconResolver::new(vec![icons.clone()], vec![settings.clone()])
        .with_revalidate_interval(std::time::Duration::ZERO);
    let names = ["app".to_owned()];
    let query = IconQuery::new(&names, 48);
    assert_eq!(resolver.desktop_theme(), "Papirus");
    assert_eq!(resolver.resolve(&query, &|| false), Some(papirus.clone()));
    assert_eq!(
        resolver.resolve(
            &IconQuery {
                theme: Some("Breeze"),
                ..query
            },
            &|| false
        ),
        Some(breeze.clone())
    );
    // Cached results survive file removal until a watched directory changes.
    std::fs::rename(&papirus, temp.path().join("moved.svg")).unwrap();
    assert_eq!(resolver.resolve(&query, &|| false), Some(papirus.clone()));
    let touch = |path: &Path, seconds: u64| {
        std::fs::File::open(path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
            .unwrap();
    };
    touch(&icons.join("Papirus"), 1_000);
    assert_eq!(resolver.resolve(&query, &|| false), Some(hicolor.clone()));
    std::fs::write(&settings, "[Settings]\ngtk-icon-theme-name=Breeze\n").unwrap();
    touch(&settings, 2_000);
    assert_eq!(resolver.resolve(&query, &|| false), Some(breeze));
    // Cancelled lookups return nothing and are not cached.
    std::fs::remove_file(&settings).unwrap();
    touch(&icons, 3_000);
    assert_eq!(resolver.resolve(&query, &|| true), None);
    assert_eq!(resolver.resolve(&query, &|| false), Some(hicolor.clone()));
    assert_eq!(
        resolver.resolve_source(&IconSource::Path(hicolor.clone()), &query, &|| false),
        Some(hicolor)
    );
    assert_eq!(
        resolver.resolve_source(
            &IconSource::Path(icons.join("missing.png")),
            &query,
            &|| false
        ),
        None
    );
}
