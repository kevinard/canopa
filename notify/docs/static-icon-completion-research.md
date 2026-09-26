# Static icon completion research

Research date: 2026-09-16. Scope: approved W09 static presentation target, existing Notify asset preparation, and staged Niribar. This is an assessment, not decoder implementation or graphical acceptance evidence.

Implementation disposition: retain the existing bounded resolver for the selected
PNG/JPEG profile. Existing resolver fixtures and the [desktop acceptance](notification-desktop-acceptance.md)
are the implementation evidence; the alternative decoder and expanded comparison
ideas below remain research candidates, not implemented format claims.

## Selected profile and standards

The approved [plan](../.scratch/freedesktop-completion-review/PLAN.md) selects static visuals, local files, standards-aware lookup and scale-aware rendering. W09 expressly makes SVG optional and asks for requested-scale, inheritance, fallback, invalidation and bounded-worker fixtures. [ADR 0037](adr/0037-bounded-indexed-icon-discovery.md) deliberately supports PNG plus JPEG, explicitly disclaims full icon-format conformance, and selects theme/scale through configuration. These are the current product decisions; adding every optional format is not a prerequisite for the selected Notifications profile.

| Item | Upstream rule | Consequence for this project |
| --- | --- | --- |
| PNG | Recommended bitmap format in Icon Theme 0.13. | Required selected static support; implemented. |
| XPM | Included for backward compatibility, without SVG's optional exception. | Gap for a **full Icon Theme format-conformance** claim; documented exclusion from the current profile. |
| SVG | Explicitly optional; unsupported implementations ignore `.svg` files. | Useful interoperability extension, not a missing mandatory Notifications capability. |
| Requested size/scale | Lookup takes nominal size and scale; exact match precedes closest physical size. | W09 requires this behavior and fixtures. |
| Theme discovery | Current theme and roots are inputs to lookup. | Automatic GNOME/KDE preference discovery is not prescribed here. |
| Invalidation | Persistent caches must notice top-level directory mtime changes, allowing a five-second check interval. | Current per-prepared-frame lookup avoids a permanent stale filename cache. |

The table follows [Icon Theme 0.13, definitions, layout, lookup and implementation notes](https://specifications.freedesktop.org/icon-theme/latest/). Theme support must include terminal hicolor fallback. The specification does not prescribe a compositor-event subscription or an automatic desktop preference provider.

Notifications `icon-static` concerns displaying one frame, and is mutually exclusive with animation advertisement. It does not assert support for every filesystem encoding. [Notifications 1.3 protocol](https://specifications.freedesktop.org/notification/1.3/protocol.html). Icons/images are optional; when presented, application identity and notification-image precedence must remain distinct. [Icons and Images](https://specifications.freedesktop.org/notification/1.3/icons-and-images.html). Standard hints themselves are optional, while an advertised action-icon implementation must attempt named-icon interpretation and retain the localized accessible label. [Hints](https://specifications.freedesktop.org/notification/1.3/hints.html).

Interpretation: retain the explicit PNG/JPEG selected-profile claim unless widening the product scope. Do not describe XPM as optional *within full Icon Theme conformance*. Do not mark W09 complete solely from this scope decision: its existing fixture and graphical acceptance work still needs evidence.

## Verified local implementation and dependencies

- [Notify icons](../crates/canopa-notify/src/presentation/renderer/icons.rs): extensions `png`, `jpg`, `jpeg`; ordered roots, declared directories, recursive parents, hicolor/unthemed fallback, requested integer scale, per-frame metadata cache and bounded candidate work. Its [tests](../crates/canopa-notify/src/presentation/renderer/tests/icons.rs) already contain size/type/tie, requested scale, inheritance, changed-index/file, and unsafe-index cases. This investigation read them; it did not rerun them.
- [Notify images](../crates/canopa-notify/src/presentation/renderer/images.rs): bounded regular-descriptor reads, PNG/JPEG content detection, 16 MiB/file, 32 MiB encoded/frame, 4 Mi pixels/image, 32 MiB delivered RGBA/frame, 64 MiB decoder allocation setting, and one admitted blocking worker whose permit survives consumer cancellation.
- [Staged Niribar preparation](../.scratch/freedesktop-completion-review/niribar-work/src/external_panel/canopa_images.rs): body images only; 16 distinct paths/update, 8 MiB/file, 2048-axis decode limit, 32 MiB decoder limit, thumbnail to 512 pixels, two admitted workers. Its `image` features also include GIF/WebP. Its Iced `svg` feature does **not** make these body-image handles or Notify's resolver SVG-aware.
- Cargo source cache: `image 0.25.10`, `resvg 0.45.1`, `usvg 0.45.1`, `tiny-skia 0.11.4`, `roxmltree 0.20.0`, `libloading 0.8.9`, `x11-dl 2.21.0`. Staged Niribar's lock includes resvg/usvg; Notify's does not. No `image-extras`, standalone XPM decoder, GDK-Pixbuf Rust binding, or librsvg Rust binding was initially cached.
- Installed system packages: `librsvg 2:2.62.3-1`, `libxpm 3.5.19-1`, `gdk-pixbuf2 2.44.6-2`, `imagemagick 7.1.2.25-1`. Python GI enumerated enabled `xpm`, `legacy-xpm`, `svg` and `png` loaders. Availability here does not establish a portable release dependency.

Cargo source root: `/home/kevinard/.local/share/cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`. Versions above were read from cache/package metadata, not inferred from semver requirements.

## XPM candidate if the selected format scope expands

The maintained image-rs project publishes [image-extras 0.1.1](https://docs.rs/crate/image-extras/0.1.1), released 2026-05-02. Its XPM-only feature adds no format-specific dependency beyond `image ^0.25.8`; MSRV is 1.89.0 and license is MIT OR Apache-2.0. Upstream explicitly does not prioritize fuzzing and warns malformed data may panic. This is a candidate requiring local robustness verification, not a vetted drop-in guarantee. [Upstream project](https://github.com/image-rs/image-extras).

Verified published source: [0.1.1 crate archive](https://static.crates.io/crates/image-extras/image-extras-0.1.1.crate), saved at `/tmp/canopa-image-extras-0.1.1.crate`, SHA-256 `60d02eb2c9ccbbab470538fce34c7bc3be7b4e59268e65a3171367b296cdb842`. Version 0.1.0 was also downloaded for comparison.

Suggested API if pursued: `image_extras::xpm::XpmDecoder::new(Cursor::new(bytes))`, inspect dimensions, call `ImageDecoder::set_limits`, then `DynamicImage::from_decoder(decoder)` and convert to RGBA8. Constructor parses the header before palette/pixels. Limits account for eight-byte RGBA16 pixels plus palette entries; final RGBA8 and temporary conversion storage need separate accounting. Palette storage grows with parsed input instead of trusting an enormous declared capacity. Avoid global decoder registration; dispatch the selected format privately after existing bounded reads.

The 0.1.1 source accepts ImageMagick's optional `const` declaration, rejected in 0.1.0. It implements a deliberately restricted XPM3 color subset: no XPM2, extension records, HSV colors, alternate visuals, or arbitrary replacement palette. Therefore adopting it still needs an honest format-subset statement. These details are verified in the archive's `src/xpm/mod.rs`, especially `new`, `set_limits`, and `read_xpm_header`.

System alternatives are less direct: `XpmCreateXpmImageFromBuffer` parses without an X server, but returns palette strings/indices requiring color conversion, and its API has no caller allocation-budget parameter. The installed [XPM manual](https://www.x.org/docs/XPM/xpm.pdf) and `/usr/include/X11/xpm.h` establish that API. GDK-Pixbuf has incremental in-memory loading and size callbacks, but its format set is installation-dependent and a requested target size is not a general allocation bound. [GNOME loader API](https://api.pygobject.gnome.org/GdkPixbuf-2.0/class-PixbufLoader.html). Prefer these as reference decoders in fixture comparison, not a new FFI integration for this tranche.

## SVG candidate if explicitly selected later

Cached resvg/usvg 0.45.1 is the natural Rust renderer. A minimal adapter could parse bounded UTF-8 XML with `roxmltree::Document::parse_with_options` (`allow_dtd: false`, finite `nodes_limit`), then `usvg::Tree::from_xmltree`, and render to a checked `tiny_skia::Pixmap` using `resvg::render`. Explicitly override both image-href resolvers to return `None`; leaving `resources_dir` unset alone does not prevent local file reads. [usvg parser](https://github.com/linebender/resvg/blob/v0.45.1/crates/usvg/src/parser/mod.rs), [href resolver](https://github.com/linebender/resvg/blob/v0.45.1/crates/usvg/src/parser/image.rs).

Do not use `Tree::from_data` on unchecked input: this version transparently decompresses SVGZ with an unbounded `read_to_end`. Default `from_str` permits DTDs. Default href handling can read referenced files outside the admitted reader; data URLs can introduce nested images. Disabling resvg default features avoids unnecessary text/font/raster dependencies in Notify, but Cargo feature unification means explicit options remain necessary in Niribar. Cached source confirms these behaviors.

Choose raster dimensions from requested logical icon size and configured scale, check allocation before pixmap creation, and preserve aspect ratio. `tiny-skia` pixels are premultiplied; convert with each pixel's `demultiply()` before creating the existing straight-RGBA asset. [Pixmap contract](https://docs.rs/tiny-skia/0.11.4/tiny_skia/struct.Pixmap.html). Existing byte/pixel limits do not alone establish a tight bound on SVG filter/path work or intermediate allocations. A carefully declared restricted subset or an isolated worker with actual resource limits would need its own design and fixtures; an async timeout does not stop a running blocking renderer.

## Practical completion evidence

For the current profile, finish W09's comparison and real-surface evidence using the existing resolver: the same temporary roots should exercise selected theme versus parent, hicolor, first-index precedence, exact versus nearest physical size, scale 1/2, changed/missing files, and safe corrupt-image fallback. Record differences when comparing a maintained resolver; do not silently change precedence to match a library bug.

Graphical acceptance should show the same notification's application icon, independent main image, body image, and action icon on both relevant surfaces at supported scales. Include semitransparent pixels, long content, missing files/alt text, configuration reload, and replaced file content. Save the actual scale/settings and capture evidence. UI geometry already responds to Wayland scale through vendored `iced_layershell` viewport state; icon source selection remains the configured integer scale. Automatic selection could later query `GetScaleFactor` on resize and request a new prepared projection, keeping notification revisions unchanged. Automatic desktop-theme tracking requires a separately selected settings source and invalidation policy.

If adding XPM later, useful tests include transparency, hex and named colors, multi-character keys, ImageMagick `const`, unknown/duplicate codes, truncated rows, huge palette declarations, dimensions, cancellation and permit lifetime. If adding SVG, test external/local/data href rejection, DTD/SVGZ rejection, excessive tree/path/filter work, fractional-opacity output, viewport limits and scale-specific rasterization. None of these optional integrations were implemented by this research task.
