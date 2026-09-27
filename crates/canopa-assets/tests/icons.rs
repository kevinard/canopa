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
