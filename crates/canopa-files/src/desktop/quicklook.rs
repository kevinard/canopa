use super::{Desktop, clear_box, icon_button, listing::format_size, tab::Tab};
use crate::{
    Cancellation,
    preview::{Bitmap, HighlightKind, PreparedPreview, PreviewContent, PreviewRequest},
};
use gtk::{gdk, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::{Rc, Weak},
};

pub(super) trait PreviewOrigin {
    fn native(&self) -> &gtk::ApplicationWindow;
    fn app(&self) -> Option<Rc<Desktop>>;
    fn preview_key(
        self: Rc<Self>,
        key: gdk::Key,
        modifiers: gdk::ModifierType,
    ) -> glib::Propagation;
    fn preview_closed(&self, native: &gtk::Window);
}

pub(super) struct QuickLook {
    pub native: gtk::Window,
    owner: Weak<dyn PreviewOrigin>,
    body: gtk::Box,
    information: gtk::Label,
    path: RefCell<Option<PathBuf>>,
    name: RefCell<Option<String>>,
    page: Cell<u32>,
    pages: Cell<u32>,
    previous: gtk::Button,
    next: gtk::Button,
    page_label: gtk::Label,
    revision: Cell<u64>,
    cancellation: RefCell<Cancellation>,
    media: RefCell<Option<gtk::MediaFile>>,
    text_colors: RefCell<Vec<gtk::TextTag>>,
}

impl QuickLook {
    pub(super) fn appearance_changed(&self) {
        if let Some(app) = self.owner.upgrade().and_then(|owner| owner.app()) {
            for (tag, color) in self.text_colors.borrow().iter().zip(app.code_colors.get()) {
                tag.set_foreground_rgba(Some(&color));
            }
        }
    }
    pub fn new<T: PreviewOrigin + 'static>(owner: &Rc<T>) -> Rc<Self> {
        let origin: Rc<dyn PreviewOrigin> = owner.clone();
        let native = gtk::Window::builder()
            .transient_for(owner.native())
            .destroy_with_parent(true)
            .title("Quick Look")
            .default_width(760)
            .default_height(600)
            .build();
        if let Some(app) = owner.app() {
            native.set_application(Some(&app.application));
        }
        native.add_css_class("canopa-files");
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let header = gtk::HeaderBar::new();
        let open = icon_button("document-open-symbolic", "Open in default application");
        header.pack_start(&open);
        native.set_titlebar(Some(&header));
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_vexpand(true);
        body.set_hexpand(true);
        body.set_focusable(true);
        body.update_property(&[gtk::accessible::Property::Label("Preview content")]);
        body.add_css_class("preview-body");
        root.append(&body);
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        footer.add_css_class("status");
        let information = gtk::Label::new(None);
        information.set_hexpand(true);
        information.set_xalign(0.0);
        information.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        let previous = icon_button("go-previous-symbolic", "Previous page · Page Up");
        let next = icon_button("go-next-symbolic", "Next page · Page Down");
        let page_label = gtk::Label::new(None);
        previous.set_visible(false);
        next.set_visible(false);
        page_label.set_visible(false);
        footer.append(&information);
        footer.append(&previous);
        footer.append(&page_label);
        footer.append(&next);
        root.append(&footer);
        native.set_child(Some(&root));
        let preview = Rc::new(Self {
            native,
            owner: Rc::downgrade(&origin),
            body,
            information,
            path: RefCell::new(None),
            name: RefCell::new(None),
            page: Cell::new(1),
            pages: Cell::new(0),
            previous,
            next,
            page_label,
            revision: Cell::new(0),
            cancellation: RefCell::new(Cancellation::default()),
            media: RefCell::new(None),
            text_colors: RefCell::new(Vec::new()),
        });
        let weak = Rc::downgrade(&preview);
        preview.previous.connect_clicked(move |_| {
            if let Some(preview) = weak.upgrade() {
                preview.change_page(-1);
            }
        });
        let weak = Rc::downgrade(&preview);
        preview.next.connect_clicked(move |_| {
            if let Some(preview) = weak.upgrade() {
                preview.change_page(1);
            }
        });
        let weak = Rc::downgrade(&preview);
        open.connect_clicked(move |_| {
            if let Some(preview) = weak.upgrade()
                && let Some(path) = preview.path.borrow().as_ref()
            {
                let weak = Rc::downgrade(&preview);
                gio::AppInfo::launch_default_for_uri_async(
                    &gio::File::for_path(path).uri(),
                    gio::AppLaunchContext::NONE,
                    gio::Cancellable::NONE,
                    move |result| {
                        if let (Some(preview), Err(error)) = (weak.upgrade(), result) {
                            preview.information.set_label(&error.to_string());
                        }
                    },
                );
            }
        });
        let controller = gtk::EventControllerKey::new();
        // Native controls consume activation, seeking and text navigation first.
        // Unhandled keys keep navigating the originating browser.
        controller.set_propagation_phase(gtk::PropagationPhase::Bubble);
        let weak = Rc::downgrade(&preview);
        controller.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(preview) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let control_focused =
                GtkWindowExt::focus(&preview.native).is_some_and(|focus| focus != preview.body);
            if key == gdk::Key::space && control_focused {
                return glib::Propagation::Proceed;
            }
            match key {
                gdk::Key::Escape | gdk::Key::space => {
                    preview.native.close();
                    if let Some(owner) = preview.owner.upgrade() {
                        owner.native().present();
                    }
                    glib::Propagation::Stop
                }
                gdk::Key::Page_Up if preview.pages.get() > 0 => {
                    preview.change_page(-1);
                    glib::Propagation::Stop
                }
                gdk::Key::Page_Down if preview.pages.get() > 0 => {
                    preview.change_page(1);
                    glib::Propagation::Stop
                }
                _ => preview
                    .owner
                    .upgrade()
                    .map_or(glib::Propagation::Proceed, |owner| {
                        owner.preview_key(key, modifiers)
                    }),
            }
        });
        preview.native.add_controller(controller);
        let body = preview.body.downgrade();
        preview.native.connect_map(move |_| {
            if let Some(body) = body.upgrade() {
                body.grab_focus();
            }
        });
        let weak = Rc::downgrade(&preview);
        preview.native.connect_close_request(move |_| {
            if let Some(preview) = weak.upgrade() {
                preview.cancel();
                if let Some(owner) = preview.owner.upgrade() {
                    owner.preview_closed(&preview.native);
                }
            }
            glib::Propagation::Proceed
        });
        preview
    }

    fn cancel(&self) {
        self.cancellation.borrow().cancel();
        self.revision.set(self.revision.get().wrapping_add(1));
        if let Some(media) = self.media.borrow_mut().take() {
            media.pause();
        }
    }

    pub fn follow(self: &Rc<Self>, tab: &Tab) {
        self.follow_named(tab.preview_path(), None);
    }

    pub fn follow_named(self: &Rc<Self>, path: Option<PathBuf>, name: Option<String>) {
        if *self.path.borrow() == path && *self.name.borrow() == name {
            return;
        }
        self.cancel();
        *self.path.borrow_mut() = path;
        *self.name.borrow_mut() = name;
        self.page.set(1);
        self.pages.set(0);
        self.previous.set_visible(false);
        self.next.set_visible(false);
        self.page_label.set_visible(false);
        self.load();
    }

    fn change_page(self: &Rc<Self>, delta: i32) {
        let next = self
            .page
            .get()
            .saturating_add_signed(delta)
            .clamp(1, self.pages.get().max(1));
        if next != self.page.get() {
            self.page.set(next);
            self.load();
        }
    }

    fn load(self: &Rc<Self>) {
        self.cancel();
        self.text_colors.borrow_mut().clear();
        if GtkWindowExt::focus(&self.native)
            .is_some_and(|focus| focus == self.body || focus.is_ancestor(&self.body))
        {
            self.body.grab_focus();
        }
        clear_box(&self.body);
        let Some(path) = self.path.borrow().clone() else {
            self.body
                .append(&gtk::Label::new(Some("Select a file to preview")));
            self.information.set_label("");
            self.native.set_title(Some("Quick Look"));
            return;
        };
        let title = self.name.borrow().clone().unwrap_or_else(|| {
            path.file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned()
        });
        self.native
            .set_title(Some(&format!("{title} — Quick Look")));
        self.body.append(&gtk::Label::new(Some("Loading preview…")));
        self.information.set_label(&title);
        let cancel = Cancellation::default();
        *self.cancellation.borrow_mut() = cancel.clone();
        let revision = self.revision.get();
        let page = self.page.get();
        let name_hint = self.name.borrow().as_ref().map(std::ffi::OsString::from);
        let weak = Rc::downgrade(self);
        glib::MainContext::default().spawn_local(async move {
            let result = gio::spawn_blocking(move || {
                PreviewRequest {
                    path,
                    page,
                    name_hint,
                }
                .prepare(&cancel)
            })
            .await;
            let Some(preview) = weak
                .upgrade()
                .filter(|preview| preview.revision.get() == revision)
            else {
                return;
            };
            match result {
                Ok(Ok(prepared)) => preview.render(prepared),
                Ok(Err(error)) => {
                    clear_box(&preview.body);
                    let label = gtk::Label::new(Some(&error.to_string()));
                    label.set_wrap(true);
                    preview.body.append(&label);
                }
                Err(_) => {
                    clear_box(&preview.body);
                    preview
                        .body
                        .append(&gtk::Label::new(Some("Preview reader failed")));
                }
            }
        });
    }

    fn render(&self, prepared: PreparedPreview) {
        self.text_colors.borrow_mut().clear();
        clear_box(&self.body);
        let name = self.name.borrow().clone().unwrap_or_else(|| {
            prepared
                .path
                .file_name()
                .unwrap_or(prepared.path.as_os_str())
                .to_string_lossy()
                .into_owned()
        });
        self.information
            .set_label(&format!("{name} · {}", format_size(prepared.size)));
        let video_content = matches!(&prepared.content, PreviewContent::Video(_));
        match prepared.content {
            PreviewContent::Image(bitmap) => self.bitmap(bitmap),
            PreviewContent::Pdf {
                bitmap,
                page,
                pages,
            } => {
                self.page.set(page);
                self.pages.set(pages);
                self.previous.set_visible(true);
                self.next.set_visible(true);
                self.page_label.set_visible(true);
                self.previous.set_sensitive(page > 1);
                self.next.set_sensitive(page < pages);
                self.page_label.set_label(&format!("{page} / {pages}"));
                self.bitmap(bitmap);
            }
            PreviewContent::Text {
                text,
                language,
                truncated,
                highlights,
            } => {
                let view = gtk::TextView::new();
                view.set_editable(false);
                view.set_cursor_visible(false);
                view.set_monospace(true);
                view.set_left_margin(12);
                view.set_right_margin(12);
                view.set_top_margin(12);
                view.set_bottom_margin(12);
                view.buffer().set_text(&text);
                let buffer = view.buffer();
                let colors = self
                    .owner
                    .upgrade()
                    .and_then(|owner| owner.app())
                    .map(|app| app.code_colors.get())
                    .unwrap_or([gdk::RGBA::BLACK; 3]);
                let keyword = gtk::TextTag::builder().name("keyword").weight(700).build();
                let string = gtk::TextTag::builder()
                    .name("string")
                    .foreground_rgba(&colors[0])
                    .build();
                let number = gtk::TextTag::builder()
                    .name("number")
                    .foreground_rgba(&colors[1])
                    .build();
                let comment = gtk::TextTag::builder()
                    .name("comment")
                    .style(gtk::pango::Style::Italic)
                    .foreground_rgba(&colors[2])
                    .build();
                for tag in [&keyword, &string, &number, &comment] {
                    buffer.tag_table().add(tag);
                }
                *self.text_colors.borrow_mut() =
                    vec![string.clone(), number.clone(), comment.clone()];
                for span in highlights {
                    let tag = match span.kind {
                        HighlightKind::Keyword => &keyword,
                        HighlightKind::String => &string,
                        HighlightKind::Number => &number,
                        HighlightKind::Comment => &comment,
                    };
                    buffer.apply_tag(
                        tag,
                        &buffer.iter_at_offset(span.start as i32),
                        &buffer.iter_at_offset(span.end as i32),
                    );
                }
                let scroll = gtk::ScrolledWindow::builder()
                    .vexpand(true)
                    .hexpand(true)
                    .child(&view)
                    .build();
                self.body.append(&scroll);
                if language.is_some() || truncated {
                    let label = gtk::Label::new(Some(&format!(
                        "{}{}",
                        language.as_deref().unwrap_or("Text"),
                        if truncated { " · first 1 MiB" } else { "" }
                    )));
                    label.add_css_class("file-details");
                    self.body.append(&label);
                }
            }
            PreviewContent::Audio(source) | PreviewContent::Video(source) => {
                let stream = super::media_input::MediaInput::new(source);
                let media = gtk::MediaFile::for_input_stream(&stream);
                if video_content {
                    let video = gtk::Picture::for_paintable(&media);
                    video.set_can_shrink(true);
                    video.set_content_fit(gtk::ContentFit::Contain);
                    video.set_vexpand(true);
                    video.set_hexpand(true);
                    self.body.append(&video);
                } else {
                    let image = gtk::Image::from_icon_name("audio-x-generic-symbolic");
                    image.set_pixel_size(96);
                    image.set_vexpand(true);
                    self.body.append(&image);
                }
                self.body.append(&gtk::MediaControls::new(Some(&media)));
                let label = gtk::Label::new(None);
                label.set_wrap(true);
                self.body.append(&label);
                media_feedback(&media, &label);
                let label = label.downgrade();
                media.connect_error_notify(move |media| {
                    if let Some(label) = label.upgrade() {
                        media_feedback(media, &label);
                    }
                });
                *self.media.borrow_mut() = Some(media);
            }
            PreviewContent::Folder { items } => {
                let image = gtk::Image::from_icon_name("folder-symbolic");
                image.set_pixel_size(96);
                image.set_vexpand(true);
                self.body.append(&image);
                self.body
                    .append(&gtk::Label::new(Some(&format!("{items} items"))));
            }
            PreviewContent::Binary => {
                let image = gtk::Image::from_icon_name("text-x-generic-symbolic");
                image.set_pixel_size(96);
                image.set_vexpand(true);
                self.body.append(&image);
                self.body.append(&gtk::Label::new(Some(
                    "No preview available · open in its application",
                )));
            }
        }
    }

    fn bitmap(&self, bitmap: Bitmap) {
        let width = bitmap.width as i32;
        let height = bitmap.height as i32;
        let bytes = glib::Bytes::from_owned(bitmap.rgba);
        let texture = gdk::MemoryTexture::new(
            width,
            height,
            gdk::MemoryFormat::R8g8b8a8,
            &bytes,
            width as usize * 4,
        );
        let picture = gtk::Picture::for_paintable(&texture);
        picture.set_can_shrink(true);
        picture.set_content_fit(gtk::ContentFit::Contain);
        picture.set_hexpand(true);
        picture.set_vexpand(true);
        let scroll = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .child(&picture)
            .build();
        self.body.append(&scroll);
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        toolbar.set_halign(gtk::Align::Center);
        let less = icon_button("zoom-out-symbolic", "Zoom out");
        let fit = gtk::Button::with_label("Fit");
        let more = icon_button("zoom-in-symbolic", "Zoom in");
        let actual = gtk::Button::with_label("100%");
        toolbar.append(&less);
        toolbar.append(&fit);
        toolbar.append(&more);
        toolbar.append(&actual);
        self.body.append(&toolbar);
        let zoom = Rc::new(Cell::new(1.0f64));
        for (button, factor) in [(less, 0.8), (more, 1.25), (actual, 0.0)] {
            let picture = picture.clone();
            let zoom = zoom.clone();
            button.connect_clicked(move |_| {
                let next = if factor == 0.0 {
                    1.0
                } else {
                    (zoom.get() * factor).clamp(0.05, 8.0)
                };
                zoom.set(next);
                picture.set_can_shrink(false);
                picture.set_size_request(
                    (f64::from(width) * next).min(32768.0) as i32,
                    (f64::from(height) * next).min(32768.0) as i32,
                );
            });
        }
        fit.connect_clicked(move |_| {
            picture.set_can_shrink(true);
            picture.set_size_request(-1, -1);
            zoom.set(1.0);
        });
    }
}

fn media_feedback(media: &gtk::MediaFile, label: &gtk::Label) {
    if let Some(error) = media.error() {
        eprintln!("Canopa Files media preview: {error}");
        label.set_label("This file could not be played. Try opening it in its application.");
        label.set_visible(true);
    } else {
        label.set_visible(false);
    }
}
