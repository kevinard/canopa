use crate::dbus::sni_host::{ItemStatus, TrayIcon, TrayItem};
use crate::modules::{InteractionEvent, ModuleId, ModuleState};

/// Runtime state for the tray module -- the current list of live SNI items.
#[derive(Debug, Clone, Default)]
pub struct TrayState {
    pub items: Vec<TrayItem>,
}

pub struct TrayModule {
    id: ModuleId,
}

impl TrayModule {
    pub fn new(id: ModuleId) -> Self {
        TrayModule { id }
    }

    /// Resolve a file path for a named icon by searching XDG icon theme directories.
    ///
    /// Checks hicolor at multiple sizes (preferring sizes closest to 22px), then pixmaps.
    /// Searches both system and user icon directories. Returns `None` if not found.
    fn resolve_icon_path(name: &str) -> Option<std::path::PathBuf> {
        // Base directories to search, in priority order.
        let home = std::env::var("HOME").unwrap_or_default();
        let xdg_data_home =
            std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home}/.local/share"));
        let xdg_data_dirs = std::env::var("XDG_DATA_DIRS")
            .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());

        let mut data_dirs: Vec<String> = vec![xdg_data_home];
        data_dirs.extend(xdg_data_dirs.split(':').map(|s| s.to_string()));

        // Sizes to probe, ordered closest-to-22 first.
        let sizes = [
            "22x22", "24x24", "16x16", "32x32", "48x48", "64x64", "128x128", "256x256",
        ];
        let exts = ["png", "svg", "xpm"];
        // Categories tray icons commonly live in, in priority order.
        let categories = ["apps", "status", "devices"];

        for base in &data_dirs {
            // Scalable first (SVG scales perfectly).
            for cat in &categories {
                let svg = std::path::PathBuf::from(format!(
                    "{base}/icons/hicolor/scalable/{cat}/{name}.svg"
                ));
                if svg.exists() {
                    return Some(svg);
                }
            }
            // Fixed-size rasters.
            for size in &sizes {
                for cat in &categories {
                    for ext in &exts {
                        let p = std::path::PathBuf::from(format!(
                            "{base}/icons/hicolor/{size}/{cat}/{name}.{ext}"
                        ));
                        if p.exists() {
                            return Some(p);
                        }
                    }
                }
            }
        }

        // Pixmap fallback.
        for ext in &["png", "svg", "xpm"] {
            let p = std::path::PathBuf::from(format!("/usr/share/pixmaps/{name}.{ext}"));
            if p.exists() {
                return Some(p);
            }
        }

        None
    }

    /// Build an iced widget for one tray item icon.
    ///
    /// All widget data is owned (handles, cloned strings) so the element lifetime
    /// is not tied to the input reference.
    fn icon_widget(item: &TrayItem) -> iced::Element<'static, crate::app::Message> {
        const ICON_SIZE: u16 = 20;

        let icon_element: iced::Element<'static, crate::app::Message> = match &item.icon {
            TrayIcon::Named(name) => {
                if let Some(path) = Self::resolve_icon_path(name) {
                    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "svg" {
                        iced::widget::svg(iced::widget::svg::Handle::from_path(&path))
                            .width(ICON_SIZE as u32)
                            .height(ICON_SIZE as u32)
                            .into()
                    } else {
                        iced::widget::image(iced::widget::image::Handle::from_path(&path))
                            .width(ICON_SIZE as u32)
                            .height(ICON_SIZE as u32)
                            .into()
                    }
                } else {
                    // Icon not found -- render a single character fallback.
                    let ch = name
                        .chars()
                        .next()
                        .unwrap_or('?')
                        .to_uppercase()
                        .next()
                        .unwrap_or('?');
                    iced::widget::text(ch.to_string()).size(14).into()
                }
            }
            TrayIcon::Pixmap(pixmaps) => {
                if let Some((w, h, argb)) =
                    crate::dbus::sni_host::pick_best_pixmap(pixmaps, ICON_SIZE as i32)
                {
                    let rgba = crate::dbus::sni_host::argb_to_rgba(argb);
                    let handle = iced::widget::image::Handle::from_rgba(*w as u32, *h as u32, rgba);
                    iced::widget::image(handle)
                        .width(ICON_SIZE as u32)
                        .height(ICON_SIZE as u32)
                        .into()
                } else {
                    iced::widget::text("?").size(14).into()
                }
            }
        };

        // NeedsAttention: overlay a small accent dot.
        if item.status == ItemStatus::NeedsAttention {
            let dot = iced::widget::container(iced::widget::Space::new().width(6u32).height(6u32))
                .style(|_| iced::widget::container::Style {
                    background: Some(iced::Background::Color(crate::theme::ACCENT)),
                    border: iced::Border {
                        radius: iced::border::Radius::from(3.0),
                        ..Default::default()
                    },
                    ..Default::default()
                });
            iced::widget::stack![icon_element, dot].into()
        } else {
            icon_element
        }
    }

    fn render_icons(&self, items: &[TrayItem], horizontal: bool) -> iced::Element<'_, crate::app::Message> {
        use iced::widget::mouse_area;

        let visible: Vec<&TrayItem> = items
            .iter()
            .filter(|i| i.status != ItemStatus::Passive)
            .collect();

        if visible.is_empty() {
            return iced::widget::Space::new().into();
        }

        let icons: Vec<iced::Element<'static, crate::app::Message>> = visible
            .iter()
            .map(|item| {
                let service = item.service.clone();
                let menu_path = item.menu_path.clone();
                let icon = Self::icon_widget(item);
                mouse_area(icon)
                    .on_press(crate::app::Message::TrayActivate {
                        service: service.clone(),
                    })
                    .on_right_press(crate::app::Message::TrayMenuRequest {
                        service: service.clone(),
                        menu_path,
                    })
                    .on_middle_press(crate::app::Message::TraySecondaryActivate {
                        service: service.clone(),
                    })
                    .on_scroll(move |delta| {
                        let (delta_val, orientation) = match delta {
                            iced::mouse::ScrollDelta::Lines { x, y } => {
                                if y.abs() >= x.abs() {
                                    (y as i32, "vertical")
                                } else {
                                    (x as i32, "horizontal")
                                }
                            }
                            iced::mouse::ScrollDelta::Pixels { x, y } => {
                                if y.abs() >= x.abs() {
                                    (y as i32, "vertical")
                                } else {
                                    (x as i32, "horizontal")
                                }
                            }
                        };
                        crate::app::Message::TrayScroll {
                            service: service.clone(),
                            delta: delta_val,
                            orientation: orientation.to_string(),
                        }
                    })
                    .into()
            })
            .collect();

        if horizontal {
            iced::widget::row(icons)
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .height(iced::Length::Fill)
                .into()
        } else {
            iced::widget::column(icons)
                .spacing(8)
                .align_x(iced::Alignment::Center)
                .width(iced::Length::Fill)
                .into()
        }
    }
}

impl crate::modules::Module for TrayModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn module_type(&self) -> &str {
        "tray"
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        // Fed via Message::TrayUpdated from app.rs -- no independent subscription (AR5).
        iced::Subscription::none()
    }

    fn view_collapsed(&self, state: &ModuleState) -> iced::Element<'_, crate::app::Message> {
        let items = match state {
            ModuleState::Tray(s) => &s.items,
            _ => return iced::widget::Space::new().into(),
        };
        self.render_icons(items, false)
    }

    fn view_expanded(&self, state: &ModuleState) -> iced::Element<'_, crate::app::Message> {
        self.view_collapsed(state)
    }

    fn view_horizontal(&self, state: &ModuleState) -> iced::Element<'_, crate::app::Message> {
        let items = match state {
            ModuleState::Tray(s) => &s.items,
            _ => return iced::widget::Space::new().into(),
        };
        self.render_icons(items, true)
    }

    fn on_interaction(&self, _event: InteractionEvent) -> Option<crate::app::Message> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::Module;

    fn make_module() -> TrayModule {
        TrayModule::new(ModuleId::new("tray"))
    }

    #[test]
    fn tray_module_id_is_tray() {
        let m = make_module();
        assert_eq!(m.id().0, "tray");
        assert_eq!(m.module_type(), "tray");
    }

    #[test]
    fn tray_state_default_is_empty() {
        let state = TrayState::default();
        assert!(state.items.is_empty());
    }

    #[test]
    fn tray_state_stores_items() {
        let item = TrayItem {
            service: "test".into(),
            object_path: zbus::zvariant::OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: TrayIcon::Named("test".into()),
            tooltip: None,
            status: ItemStatus::Active,
            menu_path: None,
        };
        let state = TrayState { items: vec![item] };
        assert_eq!(state.items.len(), 1);
    }
}
