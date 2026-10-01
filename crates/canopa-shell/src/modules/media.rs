use super::data_source::{
    ModuleDataSourceIdentity, ModuleDataSourceMapper, ModuleDataSourceState, ModuleInstanceOutcome,
};
use super::render::ModuleRender;
use super::view::{IconLabel, IconLabelLayout};
use super::{Module, ModuleId, ModuleKind, ModuleState};
use crate::app::{Message, messages::ModuleRuntimeEvent};
use crate::config::{MediaConfig, Style};

pub(super) struct MediaModule {
    pub(super) id: ModuleId,
    pub(super) config: MediaConfig,
}

fn player(state: &ModuleState) -> Option<&canopa_shell::media::Player> {
    if let ModuleState::Media(player) = state {
        Some(player)
    } else {
        None
    }
}

impl Module for MediaModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }
    fn kind(&self) -> ModuleKind {
        ModuleKind::Media
    }
    fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::none()
    }
    fn data_source(&self) -> Option<&dyn ModuleDataSourceMapper> {
        Some(self)
    }
}

impl ModuleDataSourceMapper for MediaModule {
    fn data_source_identity(&self) -> ModuleDataSourceIdentity {
        ModuleDataSourceIdentity::Media
    }
    fn map_data_source_state(&self, state: &ModuleDataSourceState) -> ModuleInstanceOutcome {
        match state {
            ModuleDataSourceState::Media(snapshot) => snapshot
                .current()
                .map_or(ModuleInstanceOutcome::Empty, |p| {
                    ModuleInstanceOutcome::Visible(ModuleState::Media(p.clone()))
                }),
            _ => ModuleInstanceOutcome::Unavailable,
        }
    }
}

impl IconLabel for MediaModule {
    fn content_rotation(&self) -> crate::config::ContentRotation {
        self.config.content_rotation
    }

    fn icon(&self, state: &ModuleState) -> String {
        if player(state).is_some_and(|p| p.status == "Playing") {
            "▶"
        } else {
            "Ⅱ"
        }
        .into()
    }
    fn label(&self, state: &ModuleState) -> String {
        let label = player(state).map(|p| p.label()).unwrap_or_default();
        if label.chars().count() > 48 {
            format!("{}…", label.chars().take(47).collect::<String>())
        } else {
            label
        }
    }
    fn color(&self, state: &ModuleState) -> iced::Color {
        if player(state).is_some_and(|p| p.status == "Playing") {
            crate::theme::palette().accent
        } else {
            crate::theme::palette().muted
        }
    }
    fn styled_segments_for_layout(
        &self,
        state: &ModuleState,
        layout: IconLabelLayout,
        style: &Style,
    ) -> Option<Vec<(String, crate::config::ResolvedStyle)>> {
        let values = self
            .tooltip_values(state)
            .into_iter()
            .map(|(key, value)| (key, std::borrow::Cow::Owned(value)))
            .collect::<Vec<_>>();
        let status = player(state).map(|p| p.status.to_lowercase());
        super::view::render_format_segments(
            &self.config.format,
            layout.into(),
            &values,
            status.as_deref(),
            style,
        )
    }
}

impl ModuleRender for MediaModule {
    fn presentation(&self) -> Option<&dyn crate::modules::view::IconLabel> {
        Some(self)
    }

    fn has_content(&self, state: &ModuleState, _: crate::config::FormatMode) -> bool {
        player(state).is_some()
    }
    fn tooltip_values(&self, state: &ModuleState) -> Vec<(&'static str, String)> {
        let Some(p) = player(state) else {
            return vec![];
        };
        vec![
            ("icon", self.icon(state)),
            ("value", self.label(state)),
            ("label", p.label()),
            ("title", p.title.clone()),
            ("artist", p.artist.clone()),
            ("player", p.name.clone()),
            ("state", p.status.to_lowercase()),
        ]
    }
}

pub(crate) fn subscription() -> iced::Subscription<Message> {
    iced::Subscription::run(|| {
        iced::stream::channel(
            1,
            |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                use iced::futures::SinkExt;
                let (tx, mut rx) =
                    tokio::sync::watch::channel(canopa_shell::media::Snapshot::default());
                let work = canopa_shell::media::run(tx);
                tokio::pin!(work);
                let initial = rx.borrow_and_update().clone();
                if output
                    .send(Message::Modules(ModuleRuntimeEvent::Media(initial)))
                    .await
                    .is_err()
                {
                    return;
                }
                loop {
                    tokio::select! {
                        _ = &mut work => return,
                        change = rx.changed() => {
                            if change.is_err() { return; }
                            let snapshot = rx.borrow_and_update().clone();
                            if output.send(Message::Modules(ModuleRuntimeEvent::Media(snapshot))).await.is_err() { return; }
                        }
                    }
                }
            },
        )
    })
}
