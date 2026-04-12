use crate::modules::view::collapsed_icon;
use crate::theme;

/// Runtime state for the focused-window module.
///
/// `title = None` means no window is focused → module renders nothing (UX-DR16).
/// Populated by WindowFocusChanged / WindowsChanged / WindowOpenedOrChanged / WindowClosed
/// events via app.rs update(). Story 4.5 may set Unavailable instead.
#[derive(Debug, Clone, Default)]
pub struct FocusedWindowState {
    pub title: Option<String>,
}

/// Focused-window module: shows the focused window title, fed by niri IPC via SharedState.
pub struct FocusedWindowModule {
    id: crate::modules::ModuleId,
}

impl FocusedWindowModule {
    pub fn new(id: crate::modules::ModuleId) -> Self {
        FocusedWindowModule { id }
    }
}

impl crate::modules::Module for FocusedWindowModule {
    fn id(&self) -> crate::modules::ModuleId {
        self.id.clone()
    }

    fn module_type(&self) -> &str {
        "focused-window"
    }

    /// No independent data source — fed via Message::Niri in app.rs (same pattern as workspaces).
    fn subscription(&self) -> iced::Subscription<crate::app::Message> {
        iced::Subscription::none()
    }

    fn view_collapsed(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;

        let title = match state {
            ModuleState::FocusedWindow(s) => &s.title,
            _ => return iced::widget::Space::new().into(), // Loading → empty
        };

        // No focused window → render nothing (AC #4, UX-DR16).
        if title.is_none() {
            return iced::widget::Space::new().into();
        }

        // Window is focused → show icon placeholder (AC #2).
        // "□" (U+25A1) is a window-shape placeholder; replaced when icon system is added.
        collapsed_icon("□", theme::TEXT)
    }

    fn view_expanded(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{container, text};
        use iced::{Alignment, Length};

        let title = match state {
            ModuleState::FocusedWindow(s) => &s.title,
            _ => return iced::widget::Space::new().into(),
        };

        // No focused window → render nothing (AC #4, UX-DR16).
        let Some(title_str) = title.clone() else {
            return iced::widget::Space::new().into();
        };

        // Show title, clipped to module width (AC #3).
        // Iced clips text naturally when container width limits it.
        container(
            text(title_str)
                .size(12)
                .color(theme::TEXT)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(Length::Fill)
        .height(20)
        .align_x(Alignment::Start)
        .align_y(Alignment::Center)
        .padding([0, 6])
        .into()
    }

    fn view_horizontal(
        &self,
        state: &crate::modules::ModuleState,
    ) -> iced::Element<'_, crate::app::Message> {
        use crate::modules::ModuleState;
        use iced::widget::{container, text};
        use iced::{Alignment, Length};

        let title = match state {
            ModuleState::FocusedWindow(s) => &s.title,
            _ => return iced::widget::Space::new().into(),
        };

        let Some(title_str) = title.clone() else {
            return iced::widget::Space::new().into();
        };

        container(
            text(title_str)
                .size(12)
                .color(theme::TEXT)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Start)
        .align_y(Alignment::Center)
        .padding([0, 6])
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::Module;

    #[test]
    fn focused_window_module_id_is_focused_window() {
        let m = FocusedWindowModule::new(crate::modules::ModuleId::new("focused-window"));
        assert_eq!(m.id().0, "focused-window");
        assert_eq!(m.module_type(), "focused-window");
    }

    #[test]
    fn focused_window_state_default_has_no_title() {
        let state = FocusedWindowState::default();
        assert!(state.title.is_none());
    }

    #[test]
    fn focused_window_state_stores_title() {
        let state = FocusedWindowState {
            title: Some("Firefox".to_string()),
        };
        assert_eq!(state.title.as_deref(), Some("Firefox"));
    }
}
