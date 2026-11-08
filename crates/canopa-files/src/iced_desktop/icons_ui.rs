//! One admitted asset worker and one replaceable pending catalog. Views only
//! clone prepared handles; lookup, reads and decoding never run during layout.
use super::{chrome::Icon, *};
use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, Raster, RasterBatch, RasterBudget, RasterPolicy,
    icons::{IconFormats, IconLookup, LookupOptions},
};
use iced::{
    Alignment, Length,
    futures::channel::oneshot,
    widget::{container, image, row, text},
};
use resvg::{tiny_skia, usvg};
use std::sync::atomic::{AtomicBool, Ordering};

const MAX_NAMES: usize = 256;
const FILE_SIDE: u32 = 144;
const MAX_BYTES: usize = 256 * 1024;
const ENCODED_BUDGET: usize = 8 * 1024 * 1024;
const PIXEL_BUDGET: usize = 16 * 1024 * 1024;
const SIDE: u32 = 72;

struct Request {
    revision: u64,
    names: Vec<String>,
    foreground: [u8; 4],
    on_accent: [u8; 4],
    file_foreground: [u8; 4],
    folder_foreground: [u8; 4],
    media_foreground: [u8; 4],
}

#[derive(Debug)]
pub(super) struct Prepared {
    revision: u64,
    handles: BTreeMap<String, image::Handle>,
    file_handles: BTreeMap<super::file_icon::FileIcon, image::Handle>,
    chrome_handles: BTreeMap<Icon, image::Handle>,
}

#[derive(Default)]
pub(super) struct Icons {
    revision: u64,
    running: Option<(u64, Arc<AtomicBool>)>,
    pending: Option<Request>,
    handles: BTreeMap<String, image::Handle>,
    file_handles: BTreeMap<super::file_icon::FileIcon, image::Handle>,
    chrome_handles: BTreeMap<Icon, image::Handle>,
}

impl Icons {
    pub fn refresh(&mut self, configuration: &ConfigurationSnapshot) -> Task<Message> {
        self.revision += 1;
        // Never display pixels prepared for a superseded catalog/appearance.
        self.handles.clear();
        self.file_handles.clear();
        self.chrome_handles.clear();
        let foreground = canopa_ui::resolve_color(
            &configuration.appearance,
            configuration
                .appearance
                .style(canopa_theme::UiKind::Button)
                .foreground,
        )
        .into_rgba8();
        let names = configuration
            .actions
            .iter()
            .filter_map(|action| action.icon.clone())
            .take(MAX_NAMES)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let request = Request {
            revision: self.revision,
            names,
            foreground,
            on_accent: chrome::ink(&configuration.appearance, canopa_theme::ColorRole::OnAccent)
                .into_rgba8(),
            folder_foreground: chrome::ink(
                &configuration.appearance,
                canopa_theme::ColorRole::Accent,
            )
            .into_rgba8(),
            media_foreground: chrome::ink(
                &configuration.appearance,
                canopa_theme::ColorRole::Brand,
            )
            .into_rgba8(),
            file_foreground: canopa_ui::resolve_color(
                &configuration.appearance,
                configuration
                    .appearance
                    .style(canopa_theme::UiKind::MenuRow)
                    .foreground,
            )
            .into_rgba8(),
        };
        if let Some((_, cancel)) = &self.running {
            cancel.store(true, Ordering::Release);
            self.pending = Some(request);
            Task::none()
        } else {
            self.start(request)
        }
    }

    fn start(&mut self, request: Request) -> Task<Message> {
        let revision = request.revision;
        let cancel = Arc::new(AtomicBool::new(false));
        self.running = Some((revision, cancel.clone()));
        Task::perform(
            async move {
                let (send, receive) = oneshot::channel();
                let _ = std::thread::Builder::new()
                    .name("files-action-icons".into())
                    .spawn(move || {
                        let roots = canopa_assets::icons::search_roots();
                        let prepared = prepare(request, &roots, &|| cancel.load(Ordering::Acquire));
                        let _ = send.send(prepared);
                    });
                Arc::new(receive.await.unwrap_or_else(|_| Prepared {
                    revision,
                    handles: BTreeMap::new(),
                    file_handles: BTreeMap::new(),
                    chrome_handles: BTreeMap::new(),
                }))
            },
            Message::IconsReady,
        )
    }

    // Ignore duplicate/old callbacks without disturbing the current worker.
    fn accept(&mut self, prepared: &Prepared) -> Option<Request> {
        if self.running.as_ref().map(|(revision, _)| *revision) != Some(prepared.revision) {
            return None;
        }
        self.running = None;
        if prepared.revision == self.revision {
            self.handles = prepared.handles.clone();
            self.file_handles = prepared.file_handles.clone();
            self.chrome_handles = prepared.chrome_handles.clone();
        }
        self.pending.take()
    }

    pub fn completed(&mut self, prepared: &Prepared) -> Task<Message> {
        self.accept(prepared)
            .map_or_else(Task::none, |request| self.start(request))
    }

    pub fn file(&self, entry: &crate::browse::Entry, size: f32) -> Element<'_, Message> {
        let key = super::file_icon::FileIcon::for_entry(entry);
        let content: Element<'_, Message> = self.file_handles.get(&key).map_or_else(
            || iced::widget::space().width(size).height(size).into(),
            |handle| image(handle.clone()).width(size).height(size).into(),
        );
        container(content).width(size).height(size).into()
    }

    pub(super) fn chrome(&self, icon: Icon, size: f32, opacity: f32) -> Element<'_, Message> {
        match self.chrome_handles.get(&icon) {
            Some(handle) => image(handle.clone())
                .width(size)
                .height(size)
                .opacity(opacity)
                .into(),
            None => iced::widget::space().width(size).height(size).into(),
        }
    }

    pub fn label<'a>(
        &self,
        name: Option<&str>,
        label: &'a str,
        size: f32,
        font: iced::Font,
        enabled: bool,
    ) -> Element<'a, Message> {
        let mut content = row![].spacing(6).align_y(Alignment::Center);
        if let Some(handle) = name.and_then(|name| self.handles.get(name)) {
            content = content.push(
                image(handle.clone())
                    .width(18)
                    .height(18)
                    .opacity(if enabled { 1.0 } else { 0.4 }),
            );
        }
        content
            .push(
                container(
                    text(label)
                        .size(size)
                        .font(font)
                        .wrapping(text::Wrapping::None),
                )
                .width(Length::Fill)
                .clip(true),
            )
            .into()
    }
}

impl Drop for Icons {
    fn drop(&mut self) {
        if let Some((_, cancel)) = &self.running {
            cancel.store(true, Ordering::Release);
        }
    }
}

fn prepare(request: Request, roots: &[PathBuf], cancelled: &dyn Fn() -> bool) -> Prepared {
    let mut lookup = IconLookup::new(
        roots,
        LookupOptions {
            theme: "hicolor",
            scale: 1,
            formats: IconFormats::PngSvgXpm,
            allow_unindexed_hicolor: true,
        },
        cancelled,
    );
    let mut rasters = RasterBatch::new(
        RasterPolicy {
            max_encoded_bytes: MAX_BYTES,
            max_dimension: 1024,
            max_pixels: 1024 * 1024,
            decoder_allocation: DecoderAllocation::IncludingOutput(8 * 1024 * 1024),
            formats: Formats::EnabledRasterCodecs,
            file_access: FileAccess::Follow,
            apply_orientation: false,
            thumbnail: Some(FILE_SIDE),
        },
        RasterBudget {
            file_attempts: MAX_NAMES + super::file_icon::FileIcon::ALL.len(),
            encoded_bytes: ENCODED_BUDGET,
            pixel_bytes: PIXEL_BUDGET,
        },
        cancelled,
    );
    // SVG and raster reads each have an 8 MiB batch ceiling; all output shares
    // the same RGBA admission budget. No compressed SVG or embedded resources.
    let mut svg_bytes = ENCODED_BUDGET;
    let chrome_handles = Icon::ALL
        .into_iter()
        .take_while(|_| !cancelled())
        .filter_map(|icon| {
            let ink = match icon {
                Icon::ActiveFolder | Icon::ActiveForward => request.on_accent,
                _ => request.foreground,
            };
            let raster = svg(icon.svg().as_bytes(), ink)?;
            Some((
                icon,
                image::Handle::from_rgba(raster.width, raster.height, raster.pixels),
            ))
        })
        .collect();
    let mut file_handles = BTreeMap::new();
    for icon in super::file_icon::FileIcon::ALL {
        if cancelled() {
            break;
        }
        use super::file_icon::FileIcon;
        let foreground = match icon {
            FileIcon::Folder | FileIcon::Table => request.folder_foreground,
            FileIcon::Pdf | FileIcon::Image | FileIcon::Archive => request.media_foreground,
            _ => request.file_foreground,
        };
        let name = icon.name().to_owned();
        let raster = lookup
            .resolve(std::slice::from_ref(&name), 48)
            .and_then(|path| {
                if path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
                {
                    if svg_bytes == 0 {
                        return None;
                    }
                    let read = canopa_file::read_bounded(&path, MAX_BYTES.min(svg_bytes));
                    let consumed = read
                        .as_ref()
                        .map(Vec::len)
                        .unwrap_or_else(|error| error.bytes_read);
                    svg_bytes = svg_bytes.saturating_sub(consumed);
                    read.ok()
                        .filter(|_| !cancelled())
                        .and_then(|bytes| svg_sized(&bytes, foreground, FILE_SIDE))
                        .filter(|raster| {
                            rasters.admit_rgba(raster.width, raster.height, raster.pixels.len())
                        })
                } else {
                    rasters.load(&path)
                }
            })
            .or_else(|| {
                svg_sized(icon.fallback(), foreground, FILE_SIDE).filter(|raster| {
                    rasters.admit_rgba(raster.width, raster.height, raster.pixels.len())
                })
            });
        if let Some(mut raster) = raster.filter(|_| !cancelled()) {
            for pixel in raster.pixels.chunks_exact_mut(4) {
                pixel[..3].copy_from_slice(&foreground[..3]);
                pixel[3] = ((u16::from(pixel[3]) * u16::from(foreground[3])) / 255) as u8;
            }
            file_handles.insert(
                icon,
                image::Handle::from_rgba(raster.width, raster.height, raster.pixels),
            );
        }
    }
    let mut handles = BTreeMap::new();
    for name in request.names.into_iter().take(MAX_NAMES) {
        if cancelled() {
            break;
        }
        let Some(path) = lookup.resolve(std::slice::from_ref(&name), 22) else {
            continue;
        };
        let raster = if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
        {
            if svg_bytes == 0 {
                continue;
            }
            let read = canopa_file::read_bounded(&path, MAX_BYTES.min(svg_bytes));
            let consumed = read
                .as_ref()
                .map(Vec::len)
                .unwrap_or_else(|error| error.bytes_read);
            svg_bytes = svg_bytes.saturating_sub(consumed);
            read.ok()
                .filter(|_| !cancelled())
                .and_then(|bytes| svg(&bytes, request.foreground))
                .filter(|raster| {
                    rasters.admit_rgba(raster.width, raster.height, raster.pixels.len())
                })
        } else {
            rasters.load(&path)
        };
        if let Some(mut raster) = raster.filter(|_| !cancelled()) {
            if name.ends_with("-symbolic") {
                for pixel in raster.pixels.chunks_exact_mut(4) {
                    pixel[..3].copy_from_slice(&request.foreground[..3]);
                    pixel[3] =
                        ((u16::from(pixel[3]) * u16::from(request.foreground[3])) / 255) as u8;
                }
            }
            handles.insert(
                name,
                image::Handle::from_rgba(raster.width, raster.height, raster.pixels),
            );
        }
    }
    Prepared {
        revision: request.revision,
        handles,
        file_handles,
        chrome_handles,
    }
}

fn svg(bytes: &[u8], foreground: [u8; 4]) -> Option<Raster> {
    svg_sized(bytes, foreground, SIDE)
}
fn svg_sized(bytes: &[u8], foreground: [u8; 4], side: u32) -> Option<Raster> {
    let source = std::str::from_utf8(bytes).ok()?;
    let [r, g, b, _] = foreground;
    let options = usvg::Options {
        // Neither image references nor dynamically discovered fonts can escape
        // the bounded icon input. Path-based theme icons need neither.
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        style_sheet: Some(format!("svg {{ color: #{r:02x}{g:02x}{b:02x}; }}")),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(source, &options).ok()?;
    let size = tree.size();
    if size.width() > 1024.0 || size.height() > 1024.0 || size.width() * size.height() > 1048576.0 {
        return None;
    }
    let scale = side as f32 / size.width().max(size.height());
    let width = (size.width() * scale).round().max(1.0) as u32;
    let height = (size.height() * scale).round().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let pixels = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let pixel = pixel.demultiply();
            [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]
        })
        .collect();
    Some(Raster {
        width,
        height,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn svg_icons_use_straight_rgba_current_color_and_do_not_load_embedded_images() {
        let raster = svg(br##"<svg xmlns="http://www.w3.org/2000/svg" width="18" height="18"><path fill="currentColor" fill-opacity=".5" d="M0 0h18v18H0z"/><image href="data:image/svg+xml,%3Csvg%20xmlns=%22http://www.w3.org/2000/svg%22%20width=%2218%22%20height=%2218%22%3E%3Cpath%20fill=%22red%22%20d=%22M0%200h18v18H0z%22/%3E%3C/svg%3E" width="18" height="18"/></svg>"##, [180,70,35,255]).unwrap();
        assert_eq!((raster.width, raster.height), (SIDE, SIDE));
        let pixel = &raster.pixels[4 * (36 * 72 + 36)..][..4];
        assert!((i16::from(pixel[0]) - 180).abs() <= 2, "{pixel:?}");
        assert!((i16::from(pixel[1]) - 70).abs() <= 2, "{pixel:?}");
        assert!((127..=129).contains(&pixel[3]));
        assert!(svg(b"\x1f\x8bcompressed", [255; 4]).is_none());
        assert!(
            svg(
                br#"<svg xmlns="http://www.w3.org/2000/svg" width="100000" height="10"/>"#,
                [255; 4]
            )
            .is_none()
        );
    }

    #[test]
    fn lookup_returns_prepared_pixels_and_missing_invalid_or_cancelled_icons_fall_back() {
        let root = tempfile::tempdir().unwrap();
        let theme = root.path().join("hicolor");
        std::fs::create_dir_all(theme.join("18x18/actions")).unwrap();
        std::fs::write(theme.join("index.theme"), "[Icon Theme]\nName=Private\nDirectories=18x18/actions\n[18x18/actions]\nSize=18\nType=Fixed\n").unwrap();
        ::image::RgbaImage::from_pixel(18, 18, ::image::Rgba([230, 30, 140, 255]))
            .save(theme.join("18x18/actions/valid.png"))
            .unwrap();
        let request = || Request {
            revision: 7,
            foreground: [255; 4],
            on_accent: [255; 4],
            file_foreground: [255; 4],
            folder_foreground: [255; 4],
            media_foreground: [255; 4],
            names: vec!["valid".into(), "missing".into(), "../invalid".into()],
        };
        let prepared = prepare(request(), &[root.path().to_owned()], &|| false);
        assert_eq!(prepared.revision, 7);
        assert_eq!(prepared.handles.keys().collect::<Vec<_>>(), ["valid"]);
        assert!(
            prepare(request(), &[root.path().to_owned()], &|| true)
                .handles
                .is_empty()
        );
    }

    #[test]
    fn superseded_catalog_pixels_cannot_replace_the_current_appearance_or_clear_its_worker() {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut icons = Icons {
            revision: 3,
            running: Some((1, cancel)),
            pending: Some(Request {
                revision: 3,
                names: vec!["new".into()],
                foreground: [255; 4],
                on_accent: [255; 4],
                file_foreground: [255; 4],
                folder_foreground: [255; 4],
                media_foreground: [255; 4],
            }),
            handles: BTreeMap::new(),
            file_handles: BTreeMap::new(),
            chrome_handles: BTreeMap::new(),
        };
        let old = Prepared {
            revision: 1,
            file_handles: BTreeMap::new(),
            chrome_handles: BTreeMap::new(),
            handles: BTreeMap::from([("old".into(), image::Handle::from_rgba(1, 1, vec![255; 4]))]),
        };
        let next = icons.accept(&old).unwrap();
        assert_eq!(next.revision, 3);
        assert!(icons.handles.is_empty());
        icons.running = Some((3, Arc::new(AtomicBool::new(false))));
        assert!(icons.accept(&old).is_none());
        assert_eq!(icons.running.as_ref().unwrap().0, 3);
        let current = Prepared {
            revision: 3,
            file_handles: BTreeMap::new(),
            chrome_handles: BTreeMap::new(),
            handles: BTreeMap::from([("new".into(), image::Handle::from_rgba(1, 1, vec![255; 4]))]),
        };
        assert!(icons.accept(&current).is_none());
        assert_eq!(icons.handles.keys().collect::<Vec<_>>(), ["new"]);
    }
}
