use crate::modules::ModuleId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelOrigin {
    Bar,
    Module(ModuleId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelContent {
    BarPanel,
    ModulePanel(ModuleId),
    Menu(String),
    DmenuRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelModuleEntry {
    pub id: ModuleId,
    pub zone: crate::modules::Zone,
    pub focused: bool,
}

#[derive(Debug, Clone)]
pub struct PanelTree {
    pub origin: PanelOrigin,
    pub content: PanelContent,
    pub origin_window: iced::window::Id,
    pub origin_bar_name: String,
    pub origin_output_name: String,
    pub anchor: Option<iced::Rectangle>,
    pub screen_size: (u32, u32),
}

pub fn panel_module_frame<'a>(
    content: iced::Element<'a, crate::app::Message>,
    focused: bool,
) -> iced::Element<'a, crate::app::Message> {
    use iced::widget::container;

    container(content)
        .padding([8, 10])
        .width(iced::Length::Fill)
        .style(move |_| iced::widget::container::Style {
            background: if focused {
                Some(iced::Background::Color(crate::theme::SURFACE0))
            } else {
                Some(iced::Background::Color(crate::theme::SURFACE1))
            },
            border: iced::Border {
                color: crate::theme::BORDER,
                width: 1.0,
                radius: iced::border::Radius::from(4.0),
            },
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_tree_stores_origin_metadata() {
        let window = iced::window::Id::unique();
        let tree = PanelTree {
            origin: PanelOrigin::Module(ModuleId::new("clock")),
            content: PanelContent::ModulePanel(ModuleId::new("clock")),
            origin_window: window,
            origin_bar_name: "main".into(),
            origin_output_name: "DP-1".into(),
            anchor: Some(iced::Rectangle {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            }),
            screen_size: (1920, 1080),
        };

        assert_eq!(tree.origin_window, window);
        assert_eq!(tree.origin_bar_name, "main");
        assert_eq!(tree.origin_output_name, "DP-1");
        assert_eq!(tree.screen_size, (1920, 1080));
    }
}
