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

#[derive(Debug, Clone)]
pub enum IpcEvent {
    Reload {
        responder: IpcResponder,
    },
    OpenMenu {
        name: String,
        responder: IpcResponder,
    },
    ToggleMenu {
        name: String,
        responder: IpcResponder,
    },
    DmenuSelection {
        request: crate::ipc::protocol::DmenuRequest,
        responder: IpcResponder,
    },
    DeviceKey {
        request: canopa_shell::device_keys::Request,
        responder: IpcResponder,
    },
}

impl IpcEvent {
    pub(super) fn diagnostic_label(&self) -> &'static str {
        match self {
            IpcEvent::Reload { .. } => "Ipc::Reload",
            IpcEvent::OpenMenu { .. } => "Ipc::OpenMenu",
            IpcEvent::ToggleMenu { .. } => "Ipc::ToggleMenu",
            IpcEvent::DmenuSelection { .. } => "Ipc::DmenuSelection",
            IpcEvent::DeviceKey { .. } => "Ipc::DeviceKey",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExternalPanelEvent {
    HandshakeCompleted {
        session_id: String,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        provider_name: String,
        anchor: Option<iced::Rectangle>,
        session: Option<crate::external_panel::ExternalPanelSessionHandle>,
        content: crate::surface::panel::ExternalPanelDocument,
    },
    ProviderFailed {
        session_id: String,
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        provider_name: String,
        anchor: Option<iced::Rectangle>,
        message: String,
    },
    ProviderContentReceived {
        session_id: String,
        provider_name: String,
        content: crate::surface::panel::ExternalPanelDocument,
    },
    ProviderEventSent {
        session_id: String,
        provider_name: String,
    },
    ProviderSessionFailed {
        session_id: String,
        provider_name: String,
        message: String,
    },
    ProviderSessionEnded {
        session_id: String,
        provider_name: String,
    },
    ButtonActivated {
        provider_name: String,
        element_id: String,
    },
    ListSearchChanged {
        provider_name: String,
        list_id: String,
        query: String,
    },
    ListEntryActivated {
        provider_name: String,
        list_id: String,
        entry_id: String,
    },
    ListCustomValueSubmitted {
        provider_name: String,
        list_id: String,
        value: String,
    },
    TextInputChanged {
        provider_name: String,
        element_id: String,
        value: String,
    },
    TextInputSubmitted {
        provider_name: String,
        element_id: String,
        value: String,
    },
}

impl ExternalPanelEvent {
    pub(super) fn diagnostic_label(&self) -> &'static str {
        match self {
            ExternalPanelEvent::HandshakeCompleted { .. } => "ExternalPanel::HandshakeCompleted",
            ExternalPanelEvent::ProviderFailed { .. } => "ExternalPanel::ProviderFailed",
            ExternalPanelEvent::ProviderContentReceived { .. } => {
                "ExternalPanel::ProviderContentReceived"
            }
            ExternalPanelEvent::ProviderEventSent { .. } => "ExternalPanel::ProviderEventSent",
            ExternalPanelEvent::ProviderSessionFailed { .. } => {
                "ExternalPanel::ProviderSessionFailed"
            }
            ExternalPanelEvent::ProviderSessionEnded { .. } => {
                "ExternalPanel::ProviderSessionEnded"
            }
            ExternalPanelEvent::ButtonActivated { .. } => "ExternalPanel::ButtonActivated",
            ExternalPanelEvent::ListSearchChanged { .. } => "ExternalPanel::ListSearchChanged",
            ExternalPanelEvent::ListEntryActivated { .. } => "ExternalPanel::ListEntryActivated",
            ExternalPanelEvent::ListCustomValueSubmitted { .. } => {
                "ExternalPanel::ListCustomValueSubmitted"
            }
            ExternalPanelEvent::TextInputChanged { .. } => "ExternalPanel::TextInputChanged",
            ExternalPanelEvent::TextInputSubmitted { .. } => "ExternalPanel::TextInputSubmitted",
        }
    }
}

#[derive(Debug, Clone)]
pub enum SurfaceEvent {
    PointerPressed(iced::window::Id),
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

impl SurfaceEvent {
    pub(super) fn diagnostic_label(&self) -> &'static str {
        match self {
            SurfaceEvent::PointerPressed(_) => "Surface::PointerPressed",
            SurfaceEvent::BarClick(_) => "Surface::BarClick",
            SurfaceEvent::BarRightClick(_) => "Surface::BarRightClick",
            SurfaceEvent::Tick => "Surface::Tick",
            SurfaceEvent::BarHoverEnter(_) => "Surface::BarHoverEnter",
            SurfaceEvent::BarHoverExit(_) => "Surface::BarHoverExit",
            SurfaceEvent::ModuleHoverEnter(_, _) => "Surface::ModuleHoverEnter",
            SurfaceEvent::ModuleHoverExit(_, _) => "Surface::ModuleHoverExit",
            SurfaceEvent::BarFocusGained(_) => "Surface::BarFocusGained",
            SurfaceEvent::BarFocusLost(_) => "Surface::BarFocusLost",
            SurfaceEvent::EscapePressed(_) => "Surface::EscapePressed",
            SurfaceEvent::FocusMoveNext(_) => "Surface::FocusMoveNext",
            SurfaceEvent::FocusMovePrev(_) => "Surface::FocusMovePrev",
            SurfaceEvent::FocusNextZone(_) => "Surface::FocusNextZone",
            SurfaceEvent::FocusPrevZone(_) => "Surface::FocusPrevZone",
            SurfaceEvent::KeyboardActivate(_) => "Surface::KeyboardActivate",
            SurfaceEvent::KeyboardSecondaryActivate(_) => "Surface::KeyboardSecondaryActivate",
            SurfaceEvent::ActionKeyPressed { .. } => "Surface::ActionKeyPressed",
            SurfaceEvent::BackPressed(_) => "Surface::BackPressed",
            SurfaceEvent::TogglePanel => "Surface::TogglePanel",
            SurfaceEvent::CursorMoved(_, _) => "Surface::CursorMoved",
            SurfaceEvent::WindowSized(_, _, _) => "Surface::WindowSized",
            SurfaceEvent::WindowClosed(_) => "Surface::WindowClosed",
            SurfaceEvent::TooltipDwellElapsed { .. } => "Surface::TooltipDwellElapsed",
            SurfaceEvent::CloseTooltip { .. } => "Surface::CloseTooltip",
        }
    }
}

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
    SearchChanged {
        window: iced::window::Id,
        query: String,
    },
    LayoutChanged {
        service: String,
        menu_path: OwnedObjectPath,
    },
    Open {
        service: String,
        menu_path: OwnedObjectPath,
        tree: MenuTree,
        origin_window: iced::window::Id,
    },
    Updated {
        service: String,
        menu_path: OwnedObjectPath,
        tree: MenuTree,
    },
}

impl MenuEvent {
    pub(super) fn diagnostic_label(&self) -> &'static str {
        match self {
            MenuEvent::ItemSelected { .. } => "Menu::ItemSelected",
            MenuEvent::Closed => "Menu::Closed",
            MenuEvent::ItemHoverEnter { .. } => "Menu::ItemHoverEnter",
            MenuEvent::ItemHoverExit(_) => "Menu::ItemHoverExit",
            MenuEvent::SearchChanged { .. } => "Menu::SearchChanged",
            MenuEvent::LayoutChanged { .. } => "Menu::LayoutChanged",
            MenuEvent::Open { .. } => "Menu::Open",
            MenuEvent::Updated { .. } => "Menu::Updated",
        }
    }
}

#[derive(Debug, Clone)]
pub enum PanelEvent {
    Dismiss(iced::window::Id),
    MotionReady(iced::window::Id),
    MotionScale {
        window: iced::window::Id,
        scale: f32,
    },
    MotionFinished {
        window: iced::window::Id,
        closing: bool,
    },
    IconsLoaded(super::panel_icons::Loaded),
    Launcher(super::launcher::Event),
    LauncherSearchChanged {
        session: u64,
        query: String,
    },
    ActivateLauncher(super::launcher::Target, usize),
    LauncherSetQuery {
        session: u64,
        query: String,
    },
    LauncherSelectScope {
        session: u64,
        scope: Option<super::launcher::QuickScope>,
    },
    LauncherActions(Option<super::launcher::Target>),
    FocusLauncher(super::launcher::Target),
    LauncherAction(usize),
    InstalledAppsLoaded {
        generation: u64,
        apps: Vec<crate::desktop_apps::DesktopApplication>,
    },
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
    OpenProvider {
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        provider_name: String,
        anchor: Option<iced::Rectangle>,
    },
    SearchChanged(String),
    MenuScrolled {
        y: f32,
        viewport_height: f32,
    },
    MenuScrollApplied,
    FocusMoveNext,
    FocusMovePrev,
    FocusIndex(usize),
    FocusSearchInput {
        retries: u8,
    },
    Activate,
    ActivateIndex(usize),
    ModuleInteraction {
        window: iced::window::Id,
        target: ModuleId,
        event: crate::modules::InteractionEvent,
    },
    Back,
}

impl PanelEvent {
    pub(super) fn diagnostic_label(&self) -> &'static str {
        match self {
            PanelEvent::MotionReady(_) => "Panel::MotionReady",
            PanelEvent::MotionScale { .. } => "Panel::MotionScale",
            PanelEvent::MotionFinished { .. } => "Panel::MotionFinished",
            PanelEvent::IconsLoaded(_) => "Panel::IconsLoaded",
            PanelEvent::Launcher(_) => "Panel::Launcher",
            PanelEvent::LauncherSearchChanged { .. } => "Panel::LauncherSearchChanged",
            PanelEvent::LauncherSetQuery { .. } => "Panel::LauncherSetQuery",
            PanelEvent::LauncherSelectScope { .. } => "Panel::LauncherSelectScope",
            PanelEvent::LauncherActions(..) => "Panel::LauncherActions",
            PanelEvent::ActivateLauncher(..) => "Panel::ActivateLauncher",
            PanelEvent::FocusLauncher(_) => "Panel::FocusLauncher",
            PanelEvent::LauncherAction(_) => "Panel::LauncherAction",
            PanelEvent::InstalledAppsLoaded { .. } => "Panel::InstalledAppsLoaded",
            PanelEvent::Open { .. } => "Panel::Open",
            PanelEvent::Dismiss(_) => "Panel::Dismiss",
            PanelEvent::Close => "Panel::Close",
            PanelEvent::Toggle { .. } => "Panel::Toggle",
            PanelEvent::OpenProvider { .. } => "Panel::OpenProvider",
            PanelEvent::SearchChanged(_) => "Panel::SearchChanged",
            PanelEvent::MenuScrolled { .. } => "Panel::MenuScrolled",
            PanelEvent::MenuScrollApplied => "Panel::MenuScrollApplied",
            PanelEvent::FocusMoveNext => "Panel::FocusMoveNext",
            PanelEvent::FocusMovePrev => "Panel::FocusMovePrev",
            PanelEvent::FocusIndex(_) => "Panel::FocusIndex",
            PanelEvent::FocusSearchInput { .. } => "Panel::FocusSearchInput",
            PanelEvent::Activate => "Panel::Activate",
            PanelEvent::ActivateIndex(_) => "Panel::ActivateIndex",
            PanelEvent::ModuleInteraction { .. } => "Panel::ModuleInteraction",
            PanelEvent::Back => "Panel::Back",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ModuleRuntimeEvent {
    ToggleIdleInhibitor,
    ToggleStasis,
    StasisToggleFinished(bool),
    Stasis(Option<canopa_widgets::idle::Snapshot>),
    IdleInhibitorApplied {
        window: iced::window::Id,
        requested: bool,
        available: bool,
    },
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
    Notifications(Option<canopa_notify_client::Summary>),
    Media(canopa_shell::media::Snapshot),
}

impl ModuleRuntimeEvent {
    pub(super) fn diagnostic_label(&self) -> &'static str {
        match self {
            Self::ToggleStasis => "Modules::ToggleStasis",
            Self::StasisToggleFinished(_) => "Modules::StasisToggleFinished",
            Self::Stasis(_) => "Modules::Stasis",
            Self::ToggleIdleInhibitor => "Modules::ToggleIdleInhibitor",
            Self::IdleInhibitorApplied { .. } => "Modules::IdleInhibitorApplied",
            ModuleRuntimeEvent::Module(_, event) => module_event_diagnostic_label(event),
            ModuleRuntimeEvent::Detail { .. } => "Modules::Detail",
            ModuleRuntimeEvent::ModuleClick(_, _) => "Modules::ModuleClick",
            ModuleRuntimeEvent::ModuleRightClick(_, _) => "Modules::ModuleRightClick",
            ModuleRuntimeEvent::ModuleScrollUp(_, _) => "Modules::ModuleScrollUp",
            ModuleRuntimeEvent::ModuleScrollDown(_, _) => "Modules::ModuleScrollDown",
            ModuleRuntimeEvent::Battery(event) => match event {
                crate::modules::battery::BatteryEvent::StateChanged { .. } => {
                    "Modules::Battery::StateChanged"
                }
                crate::modules::battery::BatteryEvent::Unavailable => {
                    "Modules::Battery::Unavailable"
                }
            },
            ModuleRuntimeEvent::Network(event) => match event {
                crate::modules::network::NetworkEvent::StateChanged { .. } => {
                    "Modules::Network::StateChanged"
                }
                crate::modules::network::NetworkEvent::Unavailable => {
                    "Modules::Network::Unavailable"
                }
            },
            ModuleRuntimeEvent::Media(_) => "Modules::Media",
            ModuleRuntimeEvent::Notifications(_) => "Modules::Notifications",
            ModuleRuntimeEvent::Volume(event) => match event {
                crate::modules::volume::VolumeEvent::StateChanged { .. } => {
                    "Modules::Volume::StateChanged"
                }
                crate::modules::volume::VolumeEvent::Unavailable => "Modules::Volume::Unavailable",
            },
        }
    }
}

pub(super) fn module_event_diagnostic_label(event: &ModuleEvent) -> &'static str {
    match event {
        ModuleEvent::Clock(crate::modules::clock::ClockEvent::Tick { .. }) => {
            "Modules::Module::ClockTick"
        }
        ModuleEvent::Script(crate::modules::script::ScriptEvent::Output(_)) => {
            "Modules::Module::ScriptOutput"
        }
        ModuleEvent::Script(crate::modules::script::ScriptEvent::Failed) => {
            "Modules::Module::ScriptFailed"
        }
        ModuleEvent::Button(crate::modules::button::ButtonEvent::Ready) => {
            "Modules::Module::ButtonReady"
        }
    }
}

#[derive(Debug, Clone)]
pub enum AppNiriEvent {
    Ipc(crate::niri::NiriEvent),
}

impl AppNiriEvent {
    pub(super) fn diagnostic_label(&self) -> &'static str {
        match self {
            AppNiriEvent::Ipc(crate::niri::NiriEvent::Connected) => "Niri::Connected",
            AppNiriEvent::Ipc(crate::niri::NiriEvent::Unavailable) => "Niri::Unavailable",
            AppNiriEvent::Ipc(crate::niri::NiriEvent::Event(event)) => {
                niri_event_diagnostic_label(event)
            }
        }
    }
}

pub(super) fn niri_event_diagnostic_label(event: &niri_ipc::Event) -> &'static str {
    match event {
        niri_ipc::Event::WorkspacesChanged { .. } => "Niri::WorkspacesChanged",
        niri_ipc::Event::WorkspaceUrgencyChanged { .. } => "Niri::WorkspaceUrgencyChanged",
        niri_ipc::Event::WorkspaceActivated { .. } => "Niri::WorkspaceActivated",
        niri_ipc::Event::WorkspaceActiveWindowChanged { .. } => {
            "Niri::WorkspaceActiveWindowChanged"
        }
        niri_ipc::Event::WindowFocusChanged { .. } => "Niri::WindowFocusChanged",
        niri_ipc::Event::WindowsChanged { .. } => "Niri::WindowsChanged",
        niri_ipc::Event::WindowOpenedOrChanged { .. } => "Niri::WindowOpenedOrChanged",
        niri_ipc::Event::WindowClosed { .. } => "Niri::WindowClosed",
        niri_ipc::Event::WindowFocusTimestampChanged { .. } => "Niri::WindowFocusTimestampChanged",
        niri_ipc::Event::WindowUrgencyChanged { .. } => "Niri::WindowUrgencyChanged",
        niri_ipc::Event::WindowLayoutsChanged { .. } => "Niri::WindowLayoutsChanged",
        niri_ipc::Event::KeyboardLayoutsChanged { .. } => "Niri::KeyboardLayoutsChanged",
        niri_ipc::Event::KeyboardLayoutSwitched { .. } => "Niri::KeyboardLayoutSwitched",
        niri_ipc::Event::OverviewOpenedOrClosed { .. } => "Niri::OverviewOpenedOrClosed",
        niri_ipc::Event::ConfigLoaded { .. } => "Niri::ConfigLoaded",
        niri_ipc::Event::ScreenshotCaptured { .. } => "Niri::ScreenshotCaptured",
        niri_ipc::Event::CastsChanged { .. } => "Niri::CastsChanged",
        niri_ipc::Event::CastStartedOrChanged { .. } => "Niri::CastStartedOrChanged",
        niri_ipc::Event::CastStopped { .. } => "Niri::CastStopped",
    }
}

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

impl TrayRuntimeEvent {
    pub(super) fn diagnostic_label(&self) -> &'static str {
        match self {
            TrayRuntimeEvent::Host(event) => match event {
                TrayEvent::Snapshot(_) => "Tray::Host::Snapshot",
                TrayEvent::ItemAdded(_) => "Tray::Host::ItemAdded",
                TrayEvent::ItemRemoved(_) => "Tray::Host::ItemRemoved",
            },
            TrayRuntimeEvent::Activate { .. } => "Tray::Activate",
            TrayRuntimeEvent::MenuRequest { .. } => "Tray::MenuRequest",
            TrayRuntimeEvent::MenuRequestKeyboard { .. } => "Tray::MenuRequestKeyboard",
            TrayRuntimeEvent::MenuRequestWithBounds { .. } => "Tray::MenuRequestWithBounds",
            TrayRuntimeEvent::SecondaryActivate { .. } => "Tray::SecondaryActivate",
            TrayRuntimeEvent::Scroll { .. } => "Tray::Scroll",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::script::ScriptEvent;
    use crate::modules::{ModuleEvent, ModuleId};

    #[test]
    fn diagnostic_labels_omit_user_payloads() {
        assert_eq!(
            PanelEvent::SearchChanged("private search".into()).diagnostic_label(),
            "Panel::SearchChanged"
        );
        assert_eq!(
            ModuleRuntimeEvent::Module(
                ModuleId::new("script"),
                ModuleEvent::Script(ScriptEvent::Output("private output".into())),
            )
            .diagnostic_label(),
            "Modules::Module::ScriptOutput"
        );
        assert_eq!(
            TrayRuntimeEvent::Scroll {
                service: "org.example.PrivateApp".into(),
                delta: 1,
                orientation: "vertical".into(),
            }
            .diagnostic_label(),
            "Tray::Scroll"
        );
    }

    #[test]
    fn diagnostic_labels_identify_nested_runtime_events() {
        assert_eq!(
            SurfaceEvent::WindowClosed(iced::window::Id::unique()).diagnostic_label(),
            "Surface::WindowClosed"
        );
        assert_eq!(
            ModuleRuntimeEvent::ModuleClick(ModuleId::new("clock"), iced::window::Id::unique(),)
                .diagnostic_label(),
            "Modules::ModuleClick"
        );
        assert_eq!(
            AppNiriEvent::Ipc(crate::niri::NiriEvent::Connected).diagnostic_label(),
            "Niri::Connected"
        );
    }
}
