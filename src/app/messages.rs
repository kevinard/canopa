use crate::dbus::sni::host::TrayEvent;
use crate::modules::{ModuleEvent, ModuleId};
use crate::surface::menu_popup::MenuTree;
use iced::{Point, Rectangle};
use zbus::zvariant::OwnedObjectPath;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum SurfaceEvent {
    BarClick(iced::window::Id),
    Tick,
    BarHoverEnter(iced::window::Id),
    BarHoverExit(iced::window::Id),
    ModuleHoverEnter(ModuleId, iced::window::Id),
    ModuleHoverExit(ModuleId, iced::window::Id),
    BarFocusGained(iced::window::Id),
    BarFocusLost(iced::window::Id),
    EscapePressed(iced::window::Id),
    FocusMoveNext(iced::window::Id),
    FocusMovePrev(iced::window::Id),
    FocusNextZone(iced::window::Id),
    FocusPrevZone(iced::window::Id),
    KeyboardActivate(iced::window::Id),
    KeyboardSecondaryActivate(iced::window::Id),
    ToggleExpand,
    CursorMoved(iced::window::Id, Point),
    WindowSized(iced::window::Id, u32, u32),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum MenuEvent {
    ItemSelected {
        service: String,
        menu_item_id: i32,
        menu_path: OwnedObjectPath,
    },
    Closed,
    ItemHoverEnter {
        window: iced::window::Id,
        item_id: String,
    },
    ItemHoverExit(iced::window::Id),
    LayoutChanged {
        service: String,
        menu_path: OwnedObjectPath,
    },
    Open {
        service: String,
        menu_path: OwnedObjectPath,
        tree: MenuTree,
        bar_name: String,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum ModuleRuntimeEvent {
    Module(ModuleId, ModuleEvent),
    ModuleClick(ModuleId, iced::window::Id),
    ModuleScrollUp(ModuleId, iced::window::Id),
    ModuleScrollDown(ModuleId, iced::window::Id),
    Battery(crate::modules::battery::BatteryEvent),
    Network(crate::modules::network::NetworkEvent),
    Volume(crate::modules::volume::VolumeEvent),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AppNiriEvent {
    Ipc(crate::niri::NiriEvent),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum TrayRuntimeEvent {
    Host(TrayEvent),
    Activate {
        service: String,
    },
    MenuRequest {
        service: String,
        menu_path: Option<OwnedObjectPath>,
        source_window: Option<iced::window::Id>,
    },
    MenuRequestKeyboard {
        service: String,
        menu_path: Option<OwnedObjectPath>,
        source_window: Option<iced::window::Id>,
    },
    MenuRequestWithBounds {
        service: String,
        menu_path: Option<OwnedObjectPath>,
        bounds: Option<Rectangle>,
        screen_size: (u32, u32),
        source_window: Option<iced::window::Id>,
    },
    SecondaryActivate {
        service: String,
    },
    Scroll {
        service: String,
        delta: i32,
        orientation: String,
    },
}
