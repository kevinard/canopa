use crate::dbus::sni::host::{ItemStatus, TrayIcon, TrayItem};
use crate::icon_theme::resolve_icon_path;
use crate::modules::{InteractionEvent, ModuleId, ModuleKind, ModuleState};

/// Build an iced widget for one tray item icon.
///
/// All widget data is owned so the element lifetime is `'static`.
fn icon_widget(
    item: &TrayItem,
    bar_style: &crate::config::Style,
) -> iced::Element<'static, crate::app::Message> {
    const ICON_SIZE: u16 = 20;

    let icon_element: iced::Element<'static, crate::app::Message> = match &item.icon {
        TrayIcon::Named(name) => {
            if let Some(path) = resolve_icon_path(name) {
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
            } else if let Some(fallback_path) = resolve_icon_path("application-x-executable") {
                let ext = fallback_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                if ext == "svg" {
                    iced::widget::svg(iced::widget::svg::Handle::from_path(&fallback_path))
                        .width(ICON_SIZE as u32)
                        .height(ICON_SIZE as u32)
                        .into()
                } else {
                    iced::widget::image(iced::widget::image::Handle::from_path(&fallback_path))
                        .width(ICON_SIZE as u32)
                        .height(ICON_SIZE as u32)
                        .into()
                }
            } else {
                // Both named icon and application-x-executable unavailable -- blank placeholder.
                iced::widget::Space::new()
                    .width(ICON_SIZE as u32)
                    .height(ICON_SIZE as u32)
                    .into()
            }
        }
        TrayIcon::Pixmap(pixmaps) => {
            if let Some((w, h, argb)) =
                crate::dbus::sni::host::pick_best_pixmap(pixmaps, ICON_SIZE as i32)
            {
                let rgba = crate::dbus::sni::host::argb_to_rgba(argb);
                let handle = iced::widget::image::Handle::from_rgba(*w as u32, *h as u32, rgba);
                iced::widget::image(handle)
                    .width(ICON_SIZE as u32)
                    .height(ICON_SIZE as u32)
                    .into()
            } else {
                crate::modules::view::styled_segment_text(
                    "?".to_string(),
                    14,
                    crate::theme::TEXT,
                    crate::modules::view::fallback_resolved_style(bar_style),
                )
            }
        }
    };

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

// ---------------------------------------------------------------------------
// TrayModule -- placeholder registered from config; replaced by TrayIconModules
// ---------------------------------------------------------------------------

/// Config-level placeholder for the tray. Replaced at runtime by one `TrayIconModule`
/// per active SNI item whenever `Message::TrayUpdated` fires.
pub(crate) struct TrayModule {
    id: ModuleId,
}

impl TrayModule {
    pub(crate) fn new(id: ModuleId) -> Self {
        TrayModule { id }
    }
}

impl crate::modules::Module for TrayModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Tray
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none()
    }
}

impl crate::modules::render::ModuleRender for TrayModule {
    fn render_collapsed_with_style(
        &self,
        _state: &ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn render_panel_with_style(
        &self,
        _state: &ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }
}

/// Stable widget id used to locate a tray icon's bounding box via widget operation.
/// Includes the bar window ID so FindBounds only matches within the correct bar when
/// the same tray icon is rendered in multiple bars simultaneously.
pub(crate) fn tray_icon_widget_id(window: iced::window::Id, service: &str) -> iced::widget::Id {
    iced::widget::Id::from(format!("tray-icon:{window:?}:{service}"))
}

// ---------------------------------------------------------------------------
// TrayIconModule -- one per active SNI item, acts as a full independent module
// ---------------------------------------------------------------------------

/// One tray icon, rendered as an independent bar module.
///
/// Instances are created dynamically by the `TrayUpdated` handler and inserted into
/// `App::modules` at the same zone position as the `TrayModule` placeholder they replace.
/// Each carries its item's data directly so no shared-state lookup is needed in view.
pub(crate) struct TrayIconModule {
    id: ModuleId,
    /// The SNI item this module represents (snapshot at last TrayUpdated).
    item: TrayItem,
    /// Config module name this icon was expanded from (e.g. `"tray"`).
    /// Used by `effective_nav_list` and the view zone map to locate these modules.
    config_name: String,
}

impl TrayIconModule {
    pub(crate) fn module_id(config_name: &str, runtime_id: &str) -> ModuleId {
        if config_name == "tray" {
            ModuleId(format!("tray:{runtime_id}"))
        } else {
            ModuleId(format!("tray:{config_name}:{runtime_id}"))
        }
    }

    pub(crate) fn new(item: TrayItem, config_name: String) -> Self {
        let id = Self::module_id(&config_name, &item.runtime_id());
        TrayIconModule {
            id,
            item,
            config_name,
        }
    }

    fn build_view(
        &self,
        horizontal: bool,
        window: iced::window::Id,
        bar_style: &crate::config::Style,
    ) -> iced::Element<'static, crate::app::Message> {
        use iced::widget::mouse_area;

        let icon = icon_widget(&self.item, bar_style);
        let service = self.item.runtime_id();

        // Left/right click are handled by on_interaction via the outer module machinery.
        // This mouse_area only wires up middle-click and scroll.
        let interactive: iced::Element<'static, crate::app::Message> = mouse_area(icon)
            .on_middle_press(crate::app::Message::Tray(
                crate::app::messages::TrayRuntimeEvent::SecondaryActivate {
                    service: service.clone(),
                },
            ))
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
                crate::app::Message::Tray(crate::app::messages::TrayRuntimeEvent::Scroll {
                    service: service.clone(),
                    delta: delta_val,
                    orientation: orientation.to_string(),
                })
            })
            .into();

        let widget_id = tray_icon_widget_id(window, &self.item.runtime_id());
        if horizontal {
            iced::widget::container(interactive)
                .id(widget_id)
                .align_x(iced::Alignment::Center)
                .align_y(iced::Alignment::Center)
                .height(iced::Length::Fill)
                .into()
        } else {
            iced::widget::container(interactive)
                .id(widget_id)
                .align_x(iced::Alignment::Center)
                .align_y(iced::Alignment::Center)
                .width(iced::Length::Fill)
                .into()
        }
    }
}

impl crate::modules::Module for TrayIconModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::TrayIcon
    }

    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none()
    }

    fn on_interaction(&self, event: InteractionEvent) -> Option<crate::app::Message> {
        match event {
            InteractionEvent::Clicked => Some(crate::app::Message::Tray(
                crate::app::messages::TrayRuntimeEvent::Activate {
                    service: self.item.runtime_id(),
                },
            )),
            InteractionEvent::RightClicked(window) => Some(crate::app::Message::Tray(
                crate::app::messages::TrayRuntimeEvent::MenuRequest {
                    service: self.item.runtime_id(),
                    menu_path: self.item.menu_path.clone(),
                    source_window: Some(window),
                },
            )),
            InteractionEvent::Activated(window) => Some(crate::app::Message::Tray(
                crate::app::messages::TrayRuntimeEvent::MenuRequestKeyboard {
                    service: self.item.runtime_id(),
                    menu_path: self.item.menu_path.clone(),
                    source_window: Some(window),
                },
            )),
            _ => None,
        }
    }

    fn config_name(&self) -> Option<&str> {
        Some(&self.config_name)
    }
}

impl crate::modules::render::ModuleRender for TrayIconModule {
    fn tooltip_values(&self, _state: &ModuleState) -> Vec<(&'static str, String)> {
        match self.item.tooltip.as_ref() {
            Some(t) => vec![
                ("title", t.title.clone()),
                ("description", t.description.clone()),
            ],
            None => Vec::new(),
        }
    }

    fn render_collapsed_with_style(
        &self,
        _state: &ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn render_panel_with_style(
        &self,
        _state: &ModuleState,
        _bar_style: &crate::config::Style,
    ) -> iced::Element<'_, crate::app::Message> {
        iced::widget::Space::new().into()
    }

    fn render_bar_collapsed_with_style(
        &self,
        _state: &ModuleState,
        window: iced::window::Id,
        bar_style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        Some(self.build_view(false, window, bar_style))
    }

    fn render_bar_horizontal_with_style(
        &self,
        _state: &ModuleState,
        window: iced::window::Id,
        bar_style: &crate::config::Style,
    ) -> Option<iced::Element<'_, crate::app::Message>> {
        Some(self.build_view(true, window, bar_style))
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
    fn resolve_icon_path_rejects_path_like_names() {
        assert!(resolve_icon_path("").is_none());
        assert!(resolve_icon_path(".").is_none());
        assert!(resolve_icon_path("..").is_none());
        assert!(resolve_icon_path("../app").is_none());
        assert!(resolve_icon_path("theme\\app").is_none());
    }

    #[test]
    fn tray_icon_module_id_includes_service() {
        let item = TrayItem {
            service: "org.test.App".into(),
            object_path: zbus::zvariant::OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: None,
            tooltip: None,
        };
        let m = TrayIconModule::new(item, "tray".into());
        assert_eq!(m.id().0, "tray:org.test.App");
        assert_eq!(m.module_type(), "tray_icon");
        assert_eq!(crate::modules::Module::config_name(&m), Some("tray"));
    }

    #[test]
    fn tray_icon_module_click_activates_service() {
        let item = TrayItem {
            service: "org.test.App".into(),
            object_path: zbus::zvariant::OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: None,
            tooltip: None,
        };
        let m = TrayIconModule::new(item, "tray".into());
        let msg = m.on_interaction(InteractionEvent::Clicked).unwrap();
        assert!(matches!(
            msg,
            crate::app::Message::Tray(
                crate::app::messages::TrayRuntimeEvent::Activate { service }
            ) if service == "org.test.App"
        ));
    }

    #[test]
    fn tray_icon_module_space_opens_menu() {
        let item = TrayItem {
            service: "org.test.App".into(),
            object_path: zbus::zvariant::OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: Some(
                zbus::zvariant::OwnedObjectPath::try_from("/com/canonical/dbusmenu").unwrap(),
            ),
            tooltip: None,
        };
        let m = TrayIconModule::new(item, "tray".into());
        let msg = m
            .on_interaction(InteractionEvent::Activated(iced::window::Id::unique()))
            .unwrap();
        assert!(matches!(
            msg,
            crate::app::Message::Tray(
                crate::app::messages::TrayRuntimeEvent::MenuRequestKeyboard { service, .. }
            ) if service == "org.test.App"
        ));
    }
}
