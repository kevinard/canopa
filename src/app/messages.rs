use crate::dbus::sni::host::TrayEvent;
use crate::modules::{ModuleEvent, ModuleId};
use crate::surface::menu_popup::MenuTree;
use iced::{Point, Rectangle};
use std::sync::{Arc, Mutex};
use zbus::zvariant::OwnedObjectPath;

pub type IpcResponder =
    Arc<Mutex<Option<tokio::sync::oneshot::Sender<crate::ipc::protocol::IpcResponse>>>>;

pub fn ipc_responder(
    sender: tokio::sync::oneshot::Sender<crate::ipc::protocol::IpcResponse>,
) -> IpcResponder {
    Arc::new(Mutex::new(Some(sender)))
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum IpcEvent {
    DmenuSelection {
        request: crate::ipc::protocol::DmenuRequest,
        responder: IpcResponder,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum SurfaceEvent {
    BarClick(iced::window::Id),
    BarRightClick(iced::window::Id),
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
    ActionKeyPressed {
        window: iced::window::Id,
        modifiers: iced::keyboard::Modifiers,
        key: iced::keyboard::Key,
    },
    BackPressed(iced::window::Id),
    TogglePanel,
    CursorMoved(iced::window::Id, Point),
    WindowSized(iced::window::Id, u32, u32),
    WindowClosed(iced::window::Id),
    /// Fired by the animation tick when a module's tooltip dwell delay
    /// has elapsed. Phase 4 wires the popup-open path on this.
    TooltipDwellElapsed {
        window: iced::window::Id,
        module: ModuleId,
    },
    /// Fired by the app to request closing any open tooltip popup
    /// for `window` — used on hover-exit, click, escape, focus-loss,
    /// and when a menu popup opens.
    CloseTooltip {
        window: iced::window::Id,
    },
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
    Updated {
        service: String,
        menu_path: OwnedObjectPath,
        tree: MenuTree,
    },
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum PanelEvent {
    Open {
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        content: crate::surface::panel::PanelContent,
        anchor: Option<iced::Rectangle>,
    },
    Close,
    Toggle {
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        content: crate::surface::panel::PanelContent,
        anchor: Option<iced::Rectangle>,
    },
    SearchChanged(String),
    MenuScrolled {
        y: f32,
        viewport_height: f32,
    },
    FocusMoveNext,
    FocusMovePrev,
    FocusIndex(usize),
    FocusSearchInput {
        retries: u8,
    },
    Activate,
    ActivateIndex(usize),
    Back,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum ModuleRuntimeEvent {
    Module(ModuleId, ModuleEvent),
    Detail {
        target: ModuleId,
        message: crate::modules::ModuleDetailMessage,
    },
    ModuleClick(ModuleId, iced::window::Id),
    ModuleRightClick(ModuleId, iced::window::Id),
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
