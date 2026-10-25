//! Native file clipboard interop. URI conversion happens only at this UI edge.
use super::{Desktop, jobs, tab::Tab};
use crate::transfers::{TransferKind, TransferRequest};
use gtk::{gdk, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    path::PathBuf,
    rc::Rc,
    time::Duration,
};

const GNOME: &str = "x-special/gnome-copied-files";
const URIS: &str = "text/uri-list";
const KDE: &str = "application/x-kde-cutselection";
const LIMIT: usize = 16 * 1024 * 1024;

#[derive(Default)]
pub(super) struct Clipboard {
    revision: Cell<u64>,
    cut_intent: Cell<Option<u64>>,
    intent_sequence: Cell<u64>,
    provider: RefCell<Option<gdk::ContentProvider>>,
    hold: RefCell<Option<gio::ApplicationHoldGuard>>,
    source: RefCell<Option<super::storage::Snapshot>>,
}

pub(super) fn initialize(app: &Rc<Desktop>) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let weak = Rc::downgrade(app);
    display.clipboard().connect_changed(move |clipboard| {
        let Some(app) = weak.upgrade() else { return };
        app.clipboard
            .revision
            .set(app.clipboard.revision.get().wrapping_add(1));
        let own = app
            .clipboard
            .provider
            .borrow()
            .as_ref()
            .is_some_and(|provider| clipboard.content().as_ref() == Some(provider));
        if !own {
            app.clipboard.cut_intent.set(None);
            app.clipboard.provider.borrow_mut().take();
            app.clipboard.hold.borrow_mut().take();
            app.clipboard.source.borrow_mut().take();
        }
    });
}

pub(super) fn publish(
    app: &Rc<Desktop>,
    paths: &[PathBuf],
    cut: bool,
) -> Result<(), glib::BoolError> {
    let intent = (cut && !paths.is_empty()).then(|| next_cut_intent(app));
    publish_with_intent(app, paths, cut, intent, Some(app.storage.snapshot()))
}

fn next_cut_intent(app: &Rc<Desktop>) -> u64 {
    let intent = app.clipboard.intent_sequence.get().wrapping_add(1);
    app.clipboard.intent_sequence.set(intent);
    intent
}

fn publish_with_intent(
    app: &Rc<Desktop>,
    paths: &[PathBuf],
    cut: bool,
    intent: Option<u64>,
    source: Option<super::storage::Snapshot>,
) -> Result<(), glib::BoolError> {
    let Some(display) = gdk::Display::default() else {
        return Ok(());
    };
    let clipboard = display.clipboard();
    if paths.is_empty() {
        clipboard.set_content(None::<&gdk::ContentProvider>)?;
        app.clipboard.provider.borrow_mut().take();
        app.clipboard.hold.borrow_mut().take();
        app.clipboard.source.borrow_mut().take();
        app.clipboard.cut_intent.set(None);
        return Ok(());
    }
    let uris: Vec<_> = paths
        .iter()
        .map(|path| gio::File::for_path(path).uri().to_string())
        .collect();
    let native = format!(
        "{}\n{}\n",
        if cut { "cut" } else { "copy" },
        uris.join("\n")
    );
    let list = format!("{}\r\n", uris.join("\r\n"));
    let provider = gdk::ContentProvider::new_union(&[
        gdk::ContentProvider::for_bytes(GNOME, &glib::Bytes::from_owned(native.into_bytes())),
        gdk::ContentProvider::for_bytes(URIS, &glib::Bytes::from_owned(list.into_bytes())),
        gdk::ContentProvider::for_bytes(
            KDE,
            &glib::Bytes::from_static(if cut { b"1" } else { b"0" }),
        ),
    ]);
    *app.clipboard.provider.borrow_mut() = Some(provider.clone());
    *app.clipboard.hold.borrow_mut() = Some(app.application.hold());
    app.clipboard.cut_intent.set(intent);
    *app.clipboard.source.borrow_mut() = source;
    if let Err(error) = clipboard.set_content(Some(&provider)) {
        app.clipboard.provider.borrow_mut().take();
        app.clipboard.hold.borrow_mut().take();
        app.clipboard.source.borrow_mut().take();
        app.clipboard.cut_intent.set(None);
        return Err(error);
    }
    Ok(())
}

pub(super) fn copy(tab: &Rc<Tab>, cut: bool) {
    let Some(app) = tab.window().and_then(|window| window.app.upgrade()) else {
        return;
    };
    let paths = tab.selection_paths();
    let mut source = app.storage.snapshot();
    if let Some((path, resolved)) = tab.storage_location() {
        source.bind_folder(path, resolved);
    }
    let intent = (cut && !paths.is_empty()).then(|| next_cut_intent(&app));
    if !paths.is_empty()
        && let Err(error) = publish_with_intent(&app, &paths, cut, intent, Some(source))
    {
        tab.show_error(&format!("Could not copy the selection: {error}"));
    }
}

async fn read(clipboard: &gdk::Clipboard, types: &[&str]) -> Result<(String, Vec<u8>), String> {
    let (stream, mime) = clipboard
        .read_future(types, glib::Priority::DEFAULT)
        .await
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    loop {
        let chunk = stream
            .read_bytes_future(64 * 1024, glib::Priority::DEFAULT)
            .await
            .map_err(|error| error.to_string())?;
        if chunk.is_empty() {
            break;
        }
        if bytes.len().saturating_add(chunk.len()) > LIMIT {
            return Err(String::from("The clipboard file list is too large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((mime.to_string(), bytes))
}

pub(super) fn local_path(uri: &str) -> Result<PathBuf, String> {
    let parsed =
        glib::Uri::parse(uri, glib::UriFlags::ENCODED).map_err(|error| error.to_string())?;
    if parsed.scheme() != "file"
        || parsed
            .host()
            .is_some_and(|host| !host.is_empty() && !host.eq_ignore_ascii_case("localhost"))
        || parsed.userinfo().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(String::from(
            "The file list contains a non-local file location",
        ));
    }
    gio::File::for_uri(uri)
        .path()
        .filter(|path| path.is_absolute())
        .ok_or_else(|| String::from("The file list contains an invalid file location"))
}

pub(super) fn paste(tab: &Rc<Tab>) {
    let Some(app) = tab.window().and_then(|window| window.app.upgrade()) else {
        return;
    };
    let Some(destination) = tab.path() else {
        return;
    };
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let clipboard = display.clipboard();
    let revision = app.clipboard.revision.get();
    let weak = Rc::downgrade(tab);
    let hold = app.application.hold();
    glib::MainContext::default().spawn_local(async move {
        let _hold = hold;
        let result = glib::future_with_timeout(Duration::from_secs(5), async {
            let (mime, bytes) = read(&clipboard, &[GNOME, URIS]).await?;
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| String::from("Invalid clipboard file list"))?;
            let mut lines = text.lines();
            let cut = if mime == GNOME {
                match lines.next() {
                    Some("cut") => true,
                    Some("copy") => false,
                    _ => return Err(String::from("Invalid clipboard file action")),
                }
            } else if clipboard.formats().contain_mime_type(KDE) {
                read(&clipboard, &[KDE]).await?.1 == b"1"
            } else {
                false
            };
            let paths = lines
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(local_path)
                .collect::<Result<Vec<_>, _>>()?;
            if paths.is_empty() {
                return Err(String::from("The clipboard contains no files"));
            }
            Ok((paths, cut))
        })
        .await;
        let report = |message: &str| {
            if let Some(tab) = weak.upgrade() {
                tab.show_error(message);
            } else {
                jobs::notify(&app, "Could not paste files", message, None);
            }
        };
        match result {
            Ok(Ok((sources, cut))) if app.clipboard.revision.get() == revision => {
                let source = app.clipboard.source.borrow().clone();
                let token = cut.then(|| {
                    app.clipboard.cut_intent.get().unwrap_or_else(|| {
                        let intent = next_cut_intent(&app);
                        app.clipboard.cut_intent.set(Some(intent));
                        intent
                    })
                });
                jobs::transfer(
                    &app,
                    weak.clone(),
                    TransferRequest {
                        kind: if cut {
                            TransferKind::Move
                        } else {
                            TransferKind::Copy
                        },
                        sources,
                        destination,
                    },
                    token,
                    source,
                );
            }
            Ok(Ok(_)) => report("The clipboard changed while reading it. Paste again."),
            Ok(Err(error)) => report(&format!("Could not paste files: {error}")),
            Err(_) => report("The clipboard owner did not respond. Try copying the files again."),
        }
    });
}

pub(super) fn moved(app: &Rc<Desktop>, intent: u64, remaining: &[PathBuf]) {
    if app.clipboard.cut_intent.get() == Some(intent) {
        // Never replace a clipboard that changed while a transfer was running.
        // Own partial updates change the read revision, while retaining this intent.
        let source = app.clipboard.source.borrow().clone();
        if let Err(error) = publish_with_intent(app, remaining, true, Some(intent), source) {
            eprintln!("canopa-files: could not update the moved file selection: {error}");
        }
    }
}

pub(super) fn install_menu(tab: &Rc<Tab>, panel: &gtk::Box, popup: &gtk::Popover) {
    for (label, action) in [("Cut", 0), ("Copy", 1), ("Paste", 2)] {
        let button = gtk::Button::with_label(label);
        button.add_css_class("flat");
        panel.append(&button);
        let weak = Rc::downgrade(tab);
        let popup = popup.downgrade();
        button.connect_clicked(move |_| {
            if let Some(popup) = popup.upgrade() {
                popup.popdown();
            }
            if let Some(tab) = weak.upgrade() {
                if action == 2 {
                    paste(&tab);
                } else {
                    copy(&tab, action == 0);
                }
            }
        });
    }
    panel.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
}
