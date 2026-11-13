//! One admitted asset worker and one replaceable pending catalog. Views only
//! clone prepared handles; lookup, reads and decoding never run during layout.
use super::{chrome::Icon, *};
use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, Raster, RasterBatch, RasterBudget, RasterPolicy,
    icons::{IconQuery, IconResolver},
};
use canopa_file::mime::{self, MimeDatabase};
use iced::{
    Alignment, Length,
    futures::channel::oneshot,
    widget::{container, image, row, text},
};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

const MAX_NAMES: usize = 256;
const FILE_SIDE: u32 = 144;
const MAX_BYTES: usize = 256 * 1024;
const ENCODED_BUDGET: usize = 8 * 1024 * 1024;
const PIXEL_BUDGET: usize = 16 * 1024 * 1024;
const SIDE: u32 = 72;
/// Types whose icons one worker prepares, and paths it sniffs, per request.
const MAX_TYPE_REQUEST: usize = 128;
const MAX_SNIFF_REQUEST: usize = 1024;
/// Prepared type icons and remembered sniffed paths stay bounded.
const MAX_TYPES: usize = 512;
const MAX_SNIFFED: usize = 65536;
/// The pseudo-type prefix of a folder with its own themed icon, such as
/// `x-canopa-folder/folder-download`. It never leaves this module's lookups.
pub(super) const SPECIAL_FOLDER: &str = "x-canopa-folder/";
/// The pseudo-type prefix of a sidebar place's themed icon, such as
/// `x-canopa-place/user-trash`. Unlike a folder, it has no fallback name: a
/// theme without it leaves the place its built-in glyph.
const PLACE: &str = "x-canopa-place/";

/// The icon-theme name of a built-in place, if themes draw one.
pub(super) fn place_icon_name(kind: crate::interface::PlaceKind) -> Option<&'static str> {
    use crate::interface::PlaceKind;
    Some(match kind {
        PlaceKind::Home => "user-home",
        PlaceKind::Desktop => "user-desktop",
        PlaceKind::Documents => "folder-documents",
        PlaceKind::Downloads => "folder-download",
        PlaceKind::Music => "folder-music",
        PlaceKind::Pictures => "folder-pictures",
        PlaceKind::Videos => "folder-videos",
        PlaceKind::Templates => "folder-templates",
        PlaceKind::Public => "folder-publicshare",
        PlaceKind::Filesystem => "drive-harddisk",
        PlaceKind::Trash => "user-trash",
        // Recovery is Files' own; themes have no icon for it.
        PlaceKind::Recovery => return None,
    })
}

struct Request {
    revision: u64,
    names: Vec<String>,
    foreground: [u8; 4],
    on_accent: [u8; 4],
    danger: [u8; 4],
    warning: [u8; 4],
    file_foreground: [u8; 4],
    folder_foreground: [u8; 4],
    media_foreground: [u8; 4],
}

#[derive(Debug)]
pub(super) struct Prepared {
    revision: u64,
    handles: BTreeMap<String, image::Handle>,
    file_handles: BTreeMap<super::file_icon::FileIcon, image::Handle>,
    /// The built-in folder standing on its square's lower edge, for grids.
    grid_folder: Option<image::Handle>,
    chrome_handles: BTreeMap<Icon, image::Handle>,
}

#[derive(Default)]
pub(super) struct Icons {
    revision: u64,
    running: Option<(u64, Arc<AtomicBool>)>,
    pending: Option<Request>,
    handles: BTreeMap<String, image::Handle>,
    file_handles: BTreeMap<super::file_icon::FileIcon, image::Handle>,
    grid_folder: Option<image::Handle>,
    chrome_handles: BTreeMap<Icon, image::Handle>,
    file_foreground: [u8; 4],
    /// The disc behind emblems and its edge, from the appearance.
    emblem_colors: (iced::Color, iced::Color),
    types: Types,
    pub(super) thumbnails: super::thumbnails_ui::Thumbnails,
    /// Each symbol's drawn rows: its pixel height and the row after its
    /// lowest visible pixel, so grid symbols stand on the tile's edge.
    drawn: std::sync::Mutex<HashMap<iced::advanced::image::Id, (u32, u32, u32)>>,
}

/// shared-mime-info types and their themed icons. The database and sniffed
/// types arrive from a worker; views only read them and clone handles.
#[derive(Default)]
struct Types {
    database: Option<Arc<MimeDatabase>>,
    /// Content-sniffed types, valid while the entry's modification time matches.
    sniffed: HashMap<PathBuf, (Option<SystemTime>, Arc<str>)>,
    /// `None`: the theme has no icon for this type; the built-in glyph is used.
    handles: HashMap<Arc<str>, Option<image::Handle>>,
    /// Each prepared type's readable name, such as "PNG image".
    descriptions: HashMap<Arc<str>, String>,
    /// Personal folders, by the pseudo-type that gives them their own icon.
    special: HashMap<PathBuf, Arc<str>>,
    requested: HashSet<Arc<str>>,
    requested_paths: HashSet<PathBuf>,
    running: Option<Arc<AtomicBool>>,
    fingerprint: Option<u64>,
}

struct TypeRequest {
    revision: u64,
    database: Option<Arc<MimeDatabase>>,
    types: Vec<Arc<str>>,
    sniff: Vec<(PathBuf, Option<SystemTime>)>,
    known: HashSet<Arc<str>>,
    foreground: [u8; 4],
}

#[derive(Debug)]
pub(super) struct PreparedTypes {
    revision: u64,
    database: Arc<MimeDatabase>,
    sniffed: Vec<(PathBuf, Option<SystemTime>, Arc<str>)>,
    handles: Vec<(Arc<str>, Option<image::Handle>)>,
    descriptions: Vec<(Arc<str>, String)>,
}

/// The emblems an entry shows on its icon: a link, then whether this person
/// may not read it, or may read but not change it.
pub(super) fn emblems(entry: &crate::browse::Entry) -> Vec<Icon> {
    let mut emblems = Vec::new();
    if entry.kind == crate::browse::EntryKind::Symlink {
        emblems.push(Icon::Link);
    }
    if !entry.access.readable {
        emblems.push(Icon::Lock);
    } else if !entry.access.writable {
        emblems.push(Icon::ReadOnly);
    }
    emblems
}

impl Icons {
    pub fn refresh(&mut self, configuration: &ConfigurationSnapshot) -> Task<Message> {
        self.revision += 1;
        // Never display pixels prepared for a superseded catalog/appearance.
        self.handles.clear();
        self.file_handles.clear();
        self.grid_folder = None;
        self.chrome_handles.clear();
        // Type icons follow the same appearance and theme; types stay known.
        self.types.handles.clear();
        self.types.requested.clear();
        self.types.fingerprint = None;
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
            danger: chrome::ink(&configuration.appearance, canopa_theme::ColorRole::Danger)
                .into_rgba8(),
            warning: chrome::ink(&configuration.appearance, canopa_theme::ColorRole::Warning)
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
        self.file_foreground = request.file_foreground;
        self.emblem_colors = (
            chrome::ink(&configuration.appearance, canopa_theme::ColorRole::Surface),
            chrome::ink(&configuration.appearance, canopa_theme::ColorRole::Border),
        );
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
                        let prepared = prepare(request, IconResolver::system(), &|| {
                            cancel.load(Ordering::Acquire)
                        });
                        let _ = send.send(prepared);
                    });
                Arc::new(receive.await.unwrap_or_else(|_| Prepared {
                    revision,
                    handles: BTreeMap::new(),
                    file_handles: BTreeMap::new(),
                    grid_folder: None,
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
            self.grid_folder = prepared.grid_folder.clone();
            self.chrome_handles = prepared.chrome_handles.clone();
            // Symbols drawn from the old pixels are measured again.
            self.drawn
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .clear();
        }
        self.pending.take()
    }

    pub fn completed(&mut self, prepared: &Prepared) -> Task<Message> {
        self.accept(prepared)
            .map_or_else(Task::none, |request| self.start(request))
    }

    pub fn file(&self, entry: &crate::browse::Entry, size: f32) -> Element<'_, Message> {
        self.file_in(entry, size, size)
    }

    /// `entry`'s picture in a `width` by `height` tile: thumbnails keep their
    /// proportions within it, symbols take its height. A wide grid tile lets
    /// landscape pictures fill it; both stand on its lower edge.
    pub fn file_in(
        &self,
        entry: &crate::browse::Entry,
        width: f32,
        height: f32,
    ) -> Element<'_, Message> {
        let size = width.min(height);
        let square = (width - height).abs() < 0.5;
        let (tile_width, tile_height) = (width, height);
        let content: Element<'_, Message> = match self.visual(entry, size <= 32.0) {
            // Row-sized thumbnails fill the icon square like the type icons
            // beside them; larger ones show the whole picture.
            // A hairline frame keeps light pages and sheets from dissolving
            // into a blank square on the row.
            FileVisual::Thumbnail(handle) if size <= 32.0 => {
                let [r, g, b, _] = self.file_foreground;
                let edge = iced::Color::from_rgba8(r, g, b, 0.32);
                container(
                    image(handle.clone())
                        .width(size - 2.0)
                        .height(size - 2.0)
                        .content_fit(iced::ContentFit::Cover)
                        .border_radius(2.0),
                )
                .padding(1)
                .style(move |_| {
                    container::Style::default().border(iced::Border {
                        color: edge,
                        width: 1.0,
                        radius: 3.0.into(),
                    })
                })
                .into()
            }
            // A picture keeps its proportions on a small mat: rounded, with
            // a hairline and a soft shadow, so it reads as a print, not a cut-out.
            FileVisual::Thumbnail(handle) => {
                let (width, height) = self
                    .picture(entry)
                    .map_or((size, size), |(w, h, _)| (w as f32, h as f32));
                let fit = (tile_width / width.max(1.0)).min(tile_height / height.max(1.0));
                let (width, height) = ((width * fit).round(), (height * fit).round());
                let [r, g, b, _] = self.file_foreground;
                let edge = iced::Color::from_rgba8(r, g, b, 0.22);
                container(
                    container(
                        image(handle.clone())
                            .width(width - 2.0)
                            .height(height - 2.0)
                            .content_fit(iced::ContentFit::Cover)
                            .border_radius(4.0),
                    )
                    .padding(1)
                    .style(move |_| {
                        container::Style::default()
                            .border(iced::Border {
                                color: edge,
                                width: 1.0,
                                radius: 5.0.into(),
                            })
                            .shadow(iced::Shadow {
                                color: iced::Color::BLACK.scale_alpha(0.16),
                                offset: iced::Vector::new(0.0, 1.5),
                                blur_radius: 4.0,
                            })
                    }),
                )
                // Pictures stand on the tile's lower edge, beside their name.
                .center_x(tile_width)
                .height(tile_height)
                .align_bottom(tile_height)
                .into()
            }
            // Large glyphs sit slightly inside the square so documents and
            // thumbnails read at the same optical size in a grid.
            // In a wide tile the built-in folder takes the tile's width,
            // standing on its lower edge like the pictures.
            FileVisual::Glyph(handle)
                if !square
                    && self.file_handles.get(&super::file_icon::FileIcon::Folder)
                        == Some(handle)
                    && self.grid_folder.is_some() =>
            {
                let side = tile_width.round();
                container(self.standing(self.grid_folder.clone().expect("checked above"), side))
                    .center_x(tile_width)
                    .height(tile_height)
                    .align_bottom(tile_height)
                    .clip(true)
                    .into()
            }
            FileVisual::Type(handle) | FileVisual::Glyph(handle) if size > 32.0 => {
                let inner = (size * if square { 0.84 } else { 0.94 }).round();
                let symbol: Element<'_, Message> = if square {
                    image(handle.clone()).width(inner).height(inner).into()
                } else {
                    // Theme symbols leave room under their drawing; trimmed,
                    // they stand on the tile's edge like pictures and folders.
                    self.standing(handle.clone(), inner)
                };
                let placed = container(symbol).center_x(tile_width).height(tile_height);
                if square {
                    placed.center_y(tile_height).into()
                } else {
                    placed.align_bottom(tile_height).into()
                }
            }
            FileVisual::Type(handle) | FileVisual::Glyph(handle) => {
                image(handle.clone()).width(size).height(size).into()
            }
            FileVisual::Pending => iced::widget::space().width(size).height(size).into(),
        };
        let content = container(content).width(tile_width).height(tile_height);
        let emblems = emblems(entry);
        if emblems.is_empty() {
            return content.into();
        }
        // Emblems sit on the icon's lower corner, on a small disc so they read
        // on any picture: the trailing corner first, then the leading one.
        let disc = (size * 0.46).round().clamp(11.0, 22.0);
        let (fill, edge) = self.emblem_colors;
        let badge = |icon| -> Element<'_, Message> {
            container(self.chrome(icon, (disc * 0.7).round(), 1.0))
                .center_x(disc)
                .center_y(disc)
                .style(move |_| {
                    container::Style::default()
                        .background(fill)
                        .border(iced::Border {
                            color: edge,
                            width: 1.0,
                            radius: (disc / 2.0).into(),
                        })
                })
                .into()
        };
        let mut layers = iced::widget::Stack::new()
            .width(tile_width)
            .height(tile_height)
            .push(content);
        for (index, icon) in emblems.into_iter().enumerate() {
            let placed = container(badge(icon)).width(tile_width).height(tile_height);
            layers = layers.push(if index == 0 {
                placed.align_right(tile_width).align_bottom(tile_height)
            } else {
                placed.align_left(tile_width).align_bottom(tile_height)
            });
        }
        layers.into()
    }

    /// A square symbol `side` wide without the empty rows below its drawing,
    /// so its lowest visible pixel meets the bottom of its bounds.
    fn standing(&self, handle: image::Handle, side: f32) -> Element<'_, Message> {
        let drawn = {
            let mut drawn = self
                .drawn
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            *drawn
                .entry(handle.id())
                .or_insert_with(|| drawn_rows(&handle))
        };
        let (width, height, bottom) = drawn;
        if width == 0 || height == 0 || bottom == 0 || bottom >= height {
            return image(handle).width(side).height(side).into();
        }
        let shown = (side * bottom as f32 / height as f32).round();
        image(handle)
            .crop(iced::Rectangle {
                x: 0,
                y: 0,
                width,
                height: bottom,
            })
            .width(side)
            .height(shown)
            .into()
    }

    /// The RGBA pixels that stand for `entry` at a large size (its thumbnail,
    /// type icon or built-in glyph), for pictures drawn outside a view, such
    /// as a drag icon. `None` until they are prepared.
    pub fn picture(&self, entry: &crate::browse::Entry) -> Option<(u32, u32, &[u8])> {
        let handle = match self.visual(entry, false) {
            FileVisual::Thumbnail(handle)
            | FileVisual::Type(handle)
            | FileVisual::Glyph(handle) => handle,
            FileVisual::Pending => return None,
        };
        match handle {
            image::Handle::Rgba {
                width,
                height,
                pixels,
                ..
            } => Some((*width, *height, pixels.as_ref())),
            _ => None,
        }
    }

    /// What stands for an entry, in precedence order: a thumbnail prepared for
    /// this path and modification time, then the type icon, then the built-in
    /// glyph. Thumbnails keep their aspect ratio inside the icon square.
    /// In a row, only pictures and videos show a thumbnail: a page or sheet
    /// shrunk to row size reads as a blank tile, so documents keep their type.
    fn visual(&self, entry: &crate::browse::Entry, row: bool) -> FileVisual<'_> {
        use super::file_icon::FileIcon;
        let mime = self.entry_type(entry);
        if let Some(handle) = self.thumbnails.handle(entry)
            && (!row
                || mime
                    .as_deref()
                    .is_some_and(|mime| mime.starts_with("image/") || mime.starts_with("video/")))
        {
            return FileVisual::Thumbnail(handle);
        }
        if let Some(handle) = mime
            .as_deref()
            .and_then(|mime| self.types.handles.get(mime))
            .and_then(Option::as_ref)
        {
            return FileVisual::Type(handle);
        }
        let glyph = match &mime {
            Some(mime) => FileIcon::for_type(mime, self.types.database.as_deref()),
            None => FileIcon::for_entry(entry),
        };
        self.file_handles
            .get(&glyph)
            .map_or(FileVisual::Pending, FileVisual::Glyph)
    }

    /// Mark Home and the XDG personal folders, such as Downloads, with the
    /// folder icons themes draw for them.
    pub fn set_special_folders(
        &mut self,
        folders: &crate::personal_folders::PersonalFolders,
        home: &Path,
    ) {
        let mut special: HashMap<PathBuf, Arc<str>> = folders
            .icons(home)
            .map(|(path, icon)| {
                (
                    path.to_owned(),
                    Arc::from(format!("{SPECIAL_FOLDER}{icon}")),
                )
            })
            .collect();
        special.insert(
            home.to_owned(),
            Arc::from(format!("{SPECIAL_FOLDER}user-home")),
        );
        self.types.special = special;
    }

    /// The readable name of `entry`'s type, such as "PNG image", once known.
    pub fn type_description(&self, entry: &crate::browse::Entry) -> Option<String> {
        if entry.is_directory() {
            return None;
        }
        let mime = self.entry_type(entry)?;
        self.types.descriptions.get(&mime).cloned()
    }

    /// Directories, then a sniffed type for this modification time, then a
    /// unique glob match. `None` until the database arrives or sniffing finishes.
    fn entry_type(&self, entry: &crate::browse::Entry) -> Option<Arc<str>> {
        if entry.is_directory() {
            if let Some(special) = self.types.special.get(&entry.path) {
                return Some(special.clone());
            }
            return Some(Arc::from(mime::DIRECTORY));
        }
        if let Some((modified, mime)) = self.types.sniffed.get(&entry.path)
            && *modified == entry.modified
        {
            return Some(mime.clone());
        }
        let database = self.types.database.as_ref()?;
        let mut globs = database.by_name(&entry.name());
        (globs.len() == 1).then(|| globs.remove(0))
    }

    /// Request types and icons for listed entries. The fingerprint changes when
    /// a listing is replaced; an unchanged one costs nothing.
    pub fn sync_types<'a>(
        &mut self,
        fingerprint: u64,
        entries: impl Iterator<Item = &'a crate::browse::Entry>,
    ) -> Task<Message> {
        if self.types.running.is_some() || self.types.fingerprint == Some(fingerprint) {
            return Task::none();
        }
        self.types.fingerprint = Some(fingerprint);
        let mut entries = entries.peekable();
        if entries.peek().is_none() {
            // Nothing listed: do not load the database or start a worker.
            return Task::none();
        }
        let mut request = TypeRequest {
            revision: self.revision,
            database: self.types.database.clone(),
            types: Vec::new(),
            sniff: Vec::new(),
            known: HashSet::new(),
            foreground: self.file_foreground,
        };
        if self.types.database.is_some() {
            let room = MAX_TYPES.saturating_sub(self.types.handles.len());
            for entry in entries {
                match self.entry_type(entry) {
                    Some(mime) => {
                        if !self.types.handles.contains_key(&mime)
                            && !self.types.requested.contains(&mime)
                            && request.types.len() < MAX_TYPE_REQUEST.min(room)
                        {
                            self.types.requested.insert(mime.clone());
                            request.types.push(mime);
                        }
                    }
                    None => {
                        if request.sniff.len() < MAX_SNIFF_REQUEST
                            && self.types.requested_paths.insert(entry.path.clone())
                        {
                            request.sniff.push((entry.path.clone(), entry.modified));
                        }
                    }
                }
            }
            // The sidebar's places ask for their themed icons once.
            for kind in crate::interface::PlaceKind::ALL {
                let Some(name) = place_icon_name(*kind) else {
                    continue;
                };
                for name in [name, "folder"] {
                    let mime: Arc<str> = Arc::from(format!("{PLACE}{name}"));
                    if !self.types.handles.contains_key(&mime)
                        && !self.types.requested.contains(&mime)
                        && !request.types.contains(&mime)
                    {
                        self.types.requested.insert(mime.clone());
                        request.types.push(mime);
                    }
                }
            }
            if request.types.is_empty() && request.sniff.is_empty() {
                return Task::none();
            }
            request.known = self
                .types
                .handles
                .keys()
                .chain(&self.types.requested)
                .cloned()
                .collect();
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.types.running = Some(cancel.clone());
        let revision = request.revision;
        Task::perform(
            async move {
                let (send, receive) = oneshot::channel();
                let _ = std::thread::Builder::new()
                    .name("files-type-icons".into())
                    .spawn(move || {
                        let prepared = prepare_types(request, IconResolver::system(), &|| {
                            cancel.load(Ordering::Acquire)
                        });
                        let _ = send.send(prepared);
                    });
                Arc::new(receive.await.unwrap_or_else(|_| PreparedTypes {
                    revision,
                    database: MimeDatabase::system(),
                    sniffed: Vec::new(),
                    handles: Vec::new(),
                    descriptions: Vec::new(),
                }))
            },
            Message::TypesReady,
        )
    }

    /// Install worker results; the caller's next sync requests what remains.
    pub fn types_completed(&mut self, prepared: &PreparedTypes) {
        let types = &mut self.types;
        types.running = None;
        types.fingerprint = None;
        types.database = Some(prepared.database.clone());
        if types.sniffed.len() + prepared.sniffed.len() > MAX_SNIFFED {
            types.sniffed.clear();
        }
        for (path, modified, mime) in &prepared.sniffed {
            types
                .sniffed
                .insert(path.clone(), (*modified, mime.clone()));
        }
        types.requested_paths.clear();
        types.requested.clear();
        // Icons prepared for a superseded appearance are requested again.
        if prepared.revision == self.revision {
            for (mime, handle) in &prepared.handles {
                types.handles.insert(mime.clone(), handle.clone());
            }
        }
        for (mime, description) in &prepared.descriptions {
            types.descriptions.insert(mime.clone(), description.clone());
        }
    }

    /// A sidebar place's icon from the icon theme: `name`'s, or for a folder
    /// (`path`) the icon its personal folder has, else the theme's folder.
    /// The built-in `fallback` glyph stands in until it is prepared, or when
    /// the theme has none.
    pub(super) fn place(
        &self,
        name: Option<&str>,
        path: Option<&Path>,
        fallback: Icon,
        size: f32,
        opacity: f32,
    ) -> Element<'_, Message> {
        let themed = |mime: &str| self.types.handles.get(mime).and_then(Option::as_ref);
        let special = path
            .and_then(|path| self.types.special.get(path))
            .and_then(|mime| themed(mime));
        let name = name.or(path.map(|_| "folder"));
        let handle = special.or_else(|| themed(&format!("{PLACE}{}", name?)));
        match handle {
            Some(handle) => image(handle.clone())
                .width(size)
                .height(size)
                .opacity(opacity)
                .into(),
            None => self.chrome(fallback, size, opacity),
        }
    }

    pub(super) fn chrome(&self, icon: Icon, size: f32, opacity: f32) -> Element<'_, Message> {
        match self.chrome_handles.get(&icon.directional()) {
            Some(handle) => image(handle.clone())
                .width(size)
                .height(size)
                .opacity(opacity)
                .into(),
            None => iced::widget::space().width(size).height(size).into(),
        }
    }

    /// A configured action's themed icon at `size`, or the same space empty.
    pub(super) fn named(&self, name: &str, size: f32) -> Element<'_, Message> {
        match self.handles.get(name) {
            Some(handle) => image(handle.clone()).width(size).height(size).into(),
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
        if let Some(cancel) = &self.types.running {
            cancel.store(true, Ordering::Release);
        }
    }
}

enum FileVisual<'a> {
    Thumbnail(&'a image::Handle),
    Type(&'a image::Handle),
    Glyph(&'a image::Handle),
    Pending,
}

fn type_rasters(cancelled: &dyn Fn() -> bool) -> RasterBatch<'_> {
    RasterBatch::new(
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
            file_attempts: MAX_TYPE_REQUEST * 2,
            encoded_bytes: ENCODED_BUDGET,
            pixel_bytes: PIXEL_BUDGET,
        },
        cancelled,
    )
}

fn svg_policy(side: u32, foreground: [u8; 4]) -> canopa_assets::svg::Policy {
    canopa_assets::svg::Policy {
        max_encoded_bytes: MAX_BYTES,
        max_dimension: 1024,
        max_pixels: 1024 * 1024,
        output_side: side,
        foreground: [foreground[0], foreground[1], foreground[2]],
    }
}

/// Sniff ambiguous entries, then resolve and rasterize the theme's icon for
/// each requested or newly sniffed type: the database icon, the type name,
/// then its generic icon, in the desktop theme with hicolor last.
fn prepare_types(
    request: TypeRequest,
    resolver: &IconResolver,
    cancelled: &dyn Fn() -> bool,
) -> PreparedTypes {
    let database = request.database.unwrap_or_else(MimeDatabase::system);
    let mut sniffed = Vec::new();
    for (path, modified) in request.sniff {
        if cancelled() {
            break;
        }
        let mime = database.detect_path(&path, mime::DEFAULT_SNIFF_BYTES);
        sniffed.push((path, modified, mime));
    }
    let mut types = request.types;
    for (_, _, mime) in &sniffed {
        if !request.known.contains(mime) && !types.contains(mime) && types.len() < MAX_TYPE_REQUEST
        {
            types.push(mime.clone());
        }
    }
    let mut rasters = type_rasters(cancelled);
    let vector = svg_policy(FILE_SIDE, request.foreground);
    let mut handles = Vec::new();
    let mut descriptions = Vec::new();
    for mime in types {
        if cancelled() {
            break;
        }
        // Personal folders ask for their own icon, then the plain folder.
        let names = match (mime.strip_prefix(SPECIAL_FOLDER), mime.strip_prefix(PLACE)) {
            (Some(icon), _) => vec![icon.to_owned(), "folder".to_owned()],
            (_, Some(icon)) => vec![icon.to_owned()],
            _ => {
                let content_type = gio::content_type_from_mime_type(&mime);
                if let Some(content_type) = content_type
                    && !gio::content_type_is_unknown(&content_type)
                {
                    descriptions.push((
                        mime.clone(),
                        gio::content_type_get_description(&content_type).to_string(),
                    ));
                }
                database.icon_names(&mime)
            }
        };
        let query = IconQuery {
            scale: 2,
            allow_unindexed_hicolor: true,
            ..IconQuery::new(&names, 48)
        };
        let handle = resolver
            .resolve(&query, cancelled)
            .and_then(|path| rasters.load_image_or_svg(&path, &vector))
            .map(canopa_ui::icons::handle);
        if !cancelled() {
            handles.push((mime, handle));
        }
    }
    PreparedTypes {
        revision: request.revision,
        database,
        sniffed,
        handles,
        descriptions,
    }
}

fn prepare(request: Request, resolver: &IconResolver, cancelled: &dyn Fn() -> bool) -> Prepared {
    let lookup = |name: &String, size| {
        let query = IconQuery {
            allow_unindexed_hicolor: true,
            ..IconQuery::new(std::slice::from_ref(name), size)
        };
        resolver.resolve(&query, cancelled)
    };
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
    // Raster and SVG reads share the 8 MiB encoded ceiling; all output shares
    // the same RGBA admission budget. No compressed SVG or embedded resources.
    let chrome_handles = Icon::ALL
        .into_iter()
        .take_while(|_| !cancelled())
        .filter_map(|icon| {
            let ink = match icon {
                Icon::ActiveFolder
                | Icon::ActiveForward
                | Icon::ActiveBack
                | Icon::ActiveColumns
                | Icon::ActiveList
                | Icon::ActiveGrid => request.on_accent,
                Icon::DangerTrash | Icon::Warning => request.danger,
                _ => request.foreground,
            };
            let raster = svg(icon.svg().as_bytes(), ink)?;
            Some((icon, canopa_ui::icons::handle(raster)))
        })
        .collect();
    let mut file_handles = BTreeMap::new();
    let mut grid_folder = None;
    for icon in super::file_icon::FileIcon::ALL {
        if cancelled() {
            break;
        }
        use super::file_icon::FileIcon;
        let foreground = match icon {
            // Kinds keep a small set of tints so a listing scans by type:
            // folders and spreadsheets, pictures and media, PDFs, packages and slides.
            FileIcon::Folder | FileIcon::Table => request.folder_foreground,
            FileIcon::Image
            | FileIcon::Document
            | FileIcon::Audio
            | FileIcon::Video
            | FileIcon::Code => request.media_foreground,
            FileIcon::Pdf => request.danger,
            FileIcon::Archive | FileIcon::Presentation => request.warning,
            _ => request.file_foreground,
        };
        let themed = lookup(&icon.name().to_owned(), 48)
            .and_then(|path| rasters.load_image_or_svg(&path, &svg_policy(FILE_SIDE, foreground)));
        let fallback = themed.is_none();
        let raster = themed.or_else(|| {
            svg_sized(icon.fallback(), foreground, FILE_SIDE).filter(|raster| {
                rasters.admit_rgba(raster.width, raster.height, raster.pixels.len())
            })
        });
        if let Some(mut raster) = raster.filter(|_| !cancelled()) {
            raster.recolor(foreground);
            file_handles.insert(icon, canopa_ui::icons::handle(raster));
        }
        // The built-in folder also comes standing on its square's lower edge,
        // so a grid tile can show it as wide as the pictures beside it.
        if fallback && icon == FileIcon::Folder {
            let standing = String::from_utf8_lossy(icon.fallback())
                .replacen(
                    "viewBox=\"2 2.5 44 43\">",
                    "viewBox=\"2 2.5 44 43\"><g transform=\"translate(0 5.5)\">",
                    1,
                )
                .replacen("</svg>", "</g></svg>", 1);
            if let Some(mut raster) =
                svg_sized(standing.as_bytes(), foreground, FILE_SIDE).filter(|raster| {
                    rasters.admit_rgba(raster.width, raster.height, raster.pixels.len())
                })
            {
                raster.recolor(foreground);
                grid_folder = Some(canopa_ui::icons::handle(raster));
            }
        }
    }
    let mut handles = BTreeMap::new();
    for name in request.names.into_iter().take(MAX_NAMES) {
        if cancelled() {
            break;
        }
        let Some(path) = lookup(&name, 22) else {
            continue;
        };
        let raster = rasters.load_image_or_svg(&path, &svg_policy(SIDE, request.foreground));
        if let Some(mut raster) = raster.filter(|_| !cancelled()) {
            if name.ends_with("-symbolic") {
                raster.recolor(request.foreground);
            }
            handles.insert(name, canopa_ui::icons::handle(raster));
        }
    }
    Prepared {
        revision: request.revision,
        handles,
        file_handles,
        grid_folder,
        chrome_handles,
    }
}

fn svg(bytes: &[u8], foreground: [u8; 4]) -> Option<Raster> {
    svg_sized(bytes, foreground, SIDE)
}
fn svg_sized(bytes: &[u8], foreground: [u8; 4], side: u32) -> Option<Raster> {
    // Neither image references nor fonts can escape the bounded icon input.
    canopa_assets::svg::rasterize(bytes, svg_policy(side, foreground))
}

/// An RGBA symbol's width, height and the row after its lowest pixel with
/// visible alpha; `(0, 0, 0)` for pixels it cannot read.
fn drawn_rows(handle: &image::Handle) -> (u32, u32, u32) {
    let image::Handle::Rgba {
        width,
        height,
        pixels,
        ..
    } = handle
    else {
        return (0, 0, 0);
    };
    let (width, height) = (*width, *height);
    let row = width as usize * 4;
    if row == 0 || pixels.len() < row * height as usize {
        return (0, 0, 0);
    }
    let bottom = (0..height as usize)
        .rev()
        .find(|&y| {
            pixels[y * row..(y + 1) * row]
                .chunks_exact(4)
                .any(|pixel| pixel[3] > 24)
        })
        .map_or(0, |y| y as u32 + 1);
    (width, height, bottom)
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
            danger: [255; 4],
            warning: [255; 4],
            file_foreground: [255; 4],
            folder_foreground: [255; 4],
            media_foreground: [255; 4],
            names: vec!["valid".into(), "missing".into(), "../invalid".into()],
        };
        let resolver = IconResolver::new(vec![root.path().to_owned()], Vec::new());
        let prepared = prepare(request(), &resolver, &|| false);
        assert_eq!(prepared.revision, 7);
        assert_eq!(prepared.handles.keys().collect::<Vec<_>>(), ["valid"]);
        assert!(prepare(request(), &resolver, &|| true).handles.is_empty());
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
                danger: [255; 4],
                warning: [255; 4],
                file_foreground: [255; 4],
                folder_foreground: [255; 4],
                media_foreground: [255; 4],
            }),
            handles: BTreeMap::new(),
            file_handles: BTreeMap::new(),
            grid_folder: None,
            chrome_handles: BTreeMap::new(),
            file_foreground: [255; 4],
            emblem_colors: Default::default(),
            types: Types::default(),
            thumbnails: Default::default(),
            drawn: Default::default(),
        };
        let old = Prepared {
            revision: 1,
            file_handles: BTreeMap::new(),
            grid_folder: None,
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
            grid_folder: None,
            chrome_handles: BTreeMap::new(),
            handles: BTreeMap::from([("new".into(), image::Handle::from_rgba(1, 1, vec![255; 4]))]),
        };
        assert!(icons.accept(&current).is_none());
        assert_eq!(icons.handles.keys().collect::<Vec<_>>(), ["new"]);
    }

    fn mime_fixture(root: &std::path::Path) -> Arc<MimeDatabase> {
        let dir = root.join("mime");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("globs2"),
            "50:text/x-python:*.py\n50:image/png:*.png\n50:application/x-tar:*.tar\n",
        )
        .unwrap();
        let mut magic = b"MIME-Magic\0\n[50:image/png]\n>0=".to_vec();
        magic.extend_from_slice(&4u16.to_be_bytes());
        magic.extend_from_slice(b"\x89PNG\n");
        std::fs::write(dir.join("magic"), magic).unwrap();
        std::fs::write(
            dir.join("generic-icons"),
            "text/x-python:text-x-script\napplication/x-tar:package-x-generic\n",
        )
        .unwrap();
        Arc::new(MimeDatabase::load(&[dir]))
    }

    fn entry(path: PathBuf) -> crate::browse::Entry {
        crate::browse::Entry::read(path).unwrap()
    }

    #[test]
    fn personal_folders_and_home_ask_for_their_own_themed_icons() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let downloads = home.join("Downloads");
        let other = home.join("Projects");
        for folder in [&downloads, &other] {
            std::fs::create_dir_all(folder).unwrap();
        }
        let folders = crate::personal_folders::PersonalFolders::conventional(&home).unwrap();
        let mut icons = Icons::default();
        icons.set_special_folders(&folders, &home);
        let special = |path: &std::path::Path| icons.entry_type(&entry(path.to_owned()));
        assert_eq!(
            special(&downloads).as_deref(),
            Some("x-canopa-folder/folder-download")
        );
        assert_eq!(special(&home).as_deref(), Some("x-canopa-folder/user-home"));
        assert_eq!(special(&other).as_deref(), Some(mime::DIRECTORY));
        // Their glyph, before the theme answers, is still a folder.
        assert_eq!(
            super::super::file_icon::FileIcon::for_type("x-canopa-folder/folder-download", None),
            super::super::file_icon::FileIcon::Folder
        );
    }

    #[test]
    fn emblems_mark_links_and_items_this_person_cannot_read_or_change() {
        use std::os::unix::fs::PermissionsExt;
        if rustix::process::geteuid().is_root() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let plain = root.path().join("plain.txt");
        std::fs::write(&plain, b"x").unwrap();
        assert!(emblems(&entry(plain.clone())).is_empty());
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&plain, &link).unwrap();
        assert_eq!(emblems(&entry(link.clone())), [Icon::Link]);
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(emblems(&entry(plain.clone())), [Icon::ReadOnly]);
        assert_eq!(emblems(&entry(link)), [Icon::Link, Icon::ReadOnly]);
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o000)).unwrap();
        assert_eq!(emblems(&entry(plain.clone())), [Icon::Lock]);
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
    }

    #[test]
    fn type_worker_sniffs_ambiguous_names_and_prepares_themed_type_icons() {
        let root = tempfile::tempdir().unwrap();
        let database = mime_fixture(root.path());
        let icons = root.path().join("icons");
        let theme = icons.join("hicolor");
        std::fs::create_dir_all(theme.join("48x48/mimetypes")).unwrap();
        std::fs::write(
            theme.join("index.theme"),
            "[Icon Theme]\nDirectories=48x48/mimetypes\n[48x48/mimetypes]\nSize=48\nType=Fixed\n",
        )
        .unwrap();
        for (name, pixel) in [
            ("text-x-script", [1, 2, 3, 255]),
            ("image-png", [4, 5, 6, 255]),
        ] {
            ::image::RgbaImage::from_pixel(48, 48, ::image::Rgba(pixel))
                .save(theme.join(format!("48x48/mimetypes/{name}.png")))
                .unwrap();
        }
        let photo = root.path().join("photo");
        ::image::RgbaImage::from_pixel(2, 2, ::image::Rgba([9, 9, 9, 255]))
            .save_with_format(&photo, ::image::ImageFormat::Png)
            .unwrap();
        let resolver = IconResolver::new(vec![icons], Vec::new());
        let prepared = prepare_types(
            TypeRequest {
                revision: 4,
                database: Some(database.clone()),
                types: vec!["text/x-python".into(), "application/x-tar".into()],
                sniff: vec![(photo.clone(), None)],
                known: HashSet::new(),
                foreground: [255; 4],
            },
            &resolver,
            &|| false,
        );
        assert_eq!(prepared.revision, 4);
        assert_eq!(prepared.sniffed.len(), 1);
        assert_eq!(&*prepared.sniffed[0].2, "image/png");
        let handles: HashMap<_, _> = prepared
            .handles
            .iter()
            .map(|(mime, handle)| (mime.to_string(), handle.is_some()))
            .collect();
        assert!(handles["text/x-python"], "generic icon from the theme");
        assert!(handles["image/png"], "sniffed types get icons too");
        assert!(
            !handles["application/x-tar"],
            "no theme icon: glyph fallback"
        );
        let cancelled = prepare_types(
            TypeRequest {
                revision: 5,
                database: Some(database),
                types: vec!["text/x-python".into()],
                sniff: vec![(photo, None)],
                known: HashSet::new(),
                foreground: [255; 4],
            },
            &resolver,
            &|| true,
        );
        assert!(cancelled.sniffed.is_empty() && cancelled.handles.is_empty());
    }

    #[test]
    fn entries_use_type_icons_then_type_glyphs_and_request_only_missing_work() {
        use super::super::file_icon::FileIcon;
        let root = tempfile::tempdir().unwrap();
        let database = mime_fixture(root.path());
        for name in ["script.py", "archive.tar", "noext", "photo.png"] {
            std::fs::write(root.path().join(name), "x").unwrap();
        }
        let glyph = |value| image::Handle::from_rgba(1, 1, vec![value; 4]);
        let mut icons = Icons::default();
        icons.file_handles = FileIcon::ALL
            .into_iter()
            .enumerate()
            .map(|(index, icon)| (icon, glyph(index as u8)))
            .collect();
        let script = entry(root.path().join("script.py"));
        let archive = entry(root.path().join("archive.tar"));
        let noext = entry(root.path().join("noext"));
        let folder = entry(root.path().join("mime"));
        // Without the database only the listing kind is known.
        assert!(
            matches!(icons.visual(&script, false), FileVisual::Glyph(h) if *h == icons.file_handles[&FileIcon::File])
        );
        assert!(
            matches!(icons.visual(&folder, false), FileVisual::Glyph(h) if *h == icons.file_handles[&FileIcon::Folder])
        );
        icons.types.database = Some(database.clone());
        assert!(
            matches!(icons.visual(&archive, false), FileVisual::Glyph(h) if *h == icons.file_handles[&FileIcon::Archive])
        );
        let typed = glyph(200);
        icons
            .types
            .handles
            .insert("text/x-python".into(), Some(typed.clone()));
        icons.types.handles.insert("application/x-tar".into(), None);
        assert!(matches!(icons.visual(&script, false), FileVisual::Type(h) if *h == typed));
        assert!(matches!(
            icons.visual(&archive, false),
            FileVisual::Glyph(_)
        ));

        let entries = [script.clone(), archive, noext.clone(), folder];
        let task = icons.sync_types(1, entries.iter());
        drop(task);
        assert!(icons.types.running.is_some());
        assert!(icons.types.requested.contains("inode/directory"));
        assert!(!icons.types.requested.contains("text/x-python"));
        assert!(icons.types.requested_paths.contains(&noext.path));
        // The sidebar's places ask for their themed icons with the first types.
        let places: Vec<Arc<str>> = icons
            .types
            .requested
            .iter()
            .filter(|mime| mime.starts_with(PLACE))
            .cloned()
            .collect();
        assert!(
            places
                .iter()
                .any(|mime| &**mime == "x-canopa-place/user-trash")
        );
        // Completion installs sniffed types; their modification time keys reuse.
        icons.types_completed(&PreparedTypes {
            revision: icons.revision,
            database,
            sniffed: vec![(noext.path.clone(), noext.modified, "text/x-python".into())],
            handles: std::iter::once(("inode/directory".into(), None))
                .chain(places.into_iter().map(|mime| (mime, None)))
                .collect(),
            descriptions: vec![("text/x-python".into(), "Python script".into())],
        });
        assert_eq!(
            icons.type_description(&noext).as_deref(),
            Some("Python script")
        );
        assert!(icons.types.running.is_none() && icons.types.requested.is_empty());
        assert!(matches!(icons.visual(&noext, false), FileVisual::Type(h) if *h == typed));
        assert!(icons.sync_types(2, entries.iter()).units() == 0);
        assert_eq!(icons.types.fingerprint, Some(2));
    }
}
