//! Observe full-card display using the drawing viewport, including scroll translation.
//! Iced's Sensor reports edge proximity, which is insufficient for this contract.
use super::{BannerTarget, BannerUiMessage, PendingBannerAcknowledgement};
use iced::{Element, Renderer, Theme};
use iced_layershell::reexport::core::{
    self, Clipboard, Event, Layout, Length, Rectangle, Shell, Size, Vector, Widget, layout, mouse,
    overlay, renderer,
    widget::{Operation, Tree},
};

pub(super) fn observe<'a>(
    content: Element<'a, BannerUiMessage>,
    target: BannerTarget,
    pending: Option<&'a PendingBannerAcknowledgement>,
) -> Element<'a, BannerUiMessage> {
    Element::new(DisplayObserver {
        content,
        target,
        pending,
    })
}

struct DisplayObserver<'a> {
    content: Element<'a, BannerUiMessage>,
    target: BannerTarget,
    pending: Option<&'a PendingBannerAcknowledgement>,
}

impl Widget<BannerUiMessage, Theme, Renderer> for DisplayObserver<'_> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(self.content.as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.content]);
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

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, BannerUiMessage>,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        // Iced rounds scroll translation to logical pixels. At maximum scroll,
        // fractional content can still extend by up to half a pixel; requiring
        // exact containment would leave that bottom card pending forever.
        let rounded_viewport = viewport.expand(0.5);
        if matches!(
            event,
            Event::Window(core::window::Event::RedrawRequested(_))
        ) && bounds.width > 0.0
            && bounds.height > 0.0
            && bounds.is_within(&rounded_viewport)
            && let Some(pending) = self.pending
            && pending.acknowledgement.is_pending(self.target)
        {
            shell.publish(BannerUiMessage::BannerFrame {
                rendered: pending.clone(),
                target: self.target,
            });
        }
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

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, BannerUiMessage, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
