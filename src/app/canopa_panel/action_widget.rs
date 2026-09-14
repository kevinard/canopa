//! Action widgets whose scroll and press state belong to an opaque interaction ID.
use super::*;
use iced::advanced::{
    Clipboard, Layout, Shell, Widget, layout, renderer,
    widget::{Operation, Tree, tree},
};
use iced::{Event, Rectangle, Renderer, Size, Theme, mouse};

pub(super) fn render<'a>(id: String, label: &str, provider: &str) -> Element<'a, Message> {
    action(id, provider, text(label.to_owned()).into())
}

pub(super) fn preview<'a>(id: &str, label: &str, provider: &str) -> Element<'a, Message> {
    action(
        id.to_owned(),
        provider,
        // Preserve all sender-controlled context without consuming the panel.
        container(scrollable(
            text(format!("Selected: {label}")).width(Length::Fill),
        ))
        .width(Length::Fill)
        .max_height(54)
        .into(),
    )
}

fn action<'a>(id: String, provider: &str, content: Element<'a, Message>) -> Element<'a, Message> {
    let content = button(content)
        .width(Length::Fill)
        .on_press(Message::ExternalPanel(
            ExternalPanelEvent::ButtonActivated {
                provider_name: provider.to_owned(),
                element_id: id.clone(),
            },
        ));
    Element::new(ActionWidget {
        id,
        content: content.into(),
    })
}

struct ActionWidget<'a> {
    id: String,
    content: Element<'a, Message>,
}

struct ActionState(String);

impl Widget<Message, Theme, Renderer> for ActionWidget<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ActionState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ActionState(self.id.clone()))
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.content.as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<ActionState>();
        if state.0 != self.id {
            state.0 = self.id.to_owned();
            // Reset even for a same-shape replacement. Iced's keyed column
            // preserves child state when the number of children is unchanged.
            tree.children[0] = Tree::new(self.content.as_widget());
        } else {
            tree.children[0].diff(self.content.as_widget());
        }
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }
}
