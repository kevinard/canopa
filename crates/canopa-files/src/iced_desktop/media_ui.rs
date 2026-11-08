//! Media presentation, focus and drafts. The playback host owns every decoder.
use super::*;
use crate::playback::{self, Playback, Session, Snapshot};
use iced::{
    Length,
    futures::{StreamExt, channel::mpsc},
    widget::{column, container, image, row, slider, text},
};
use preview_ui::Control;
use std::{sync::Mutex, time::Duration};

pub(super) struct Host {
    playback: Playback,
    notices: Notices,
}
impl Default for Host {
    fn default() -> Self {
        #[cfg(test)]
        let playback = Playback::silent();
        #[cfg(not(test))]
        let playback = Playback::default();
        Self::attach(playback)
    }
}
impl Host {
    pub fn attach(playback: Playback) -> Self {
        let notices = Notices(Arc::new(Mutex::new(playback.take_notices())));
        Self { playback, notices }
    }
    pub fn open(&self, source: crate::preview::MediaSource) -> State {
        match self.playback.open(source) {
            Ok(session) => State::new(session),
            Err(error) => State {
                session: None,
                snapshot: Snapshot {
                    phase: playback::Phase::Failed(error.to_string()),
                    ..Snapshot::default()
                },
                bitmap: None,
                seek_draft: None,
                error: None,
            },
        }
    }
    pub fn holds_application(&self) -> bool {
        self.playback.holds_application()
    }
    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run_with(self.notices.clone(), |key| {
            let receiver = key.0.lock().unwrap_or_else(|e| e.into_inner()).take();
            iced::futures::stream::unfold(receiver, |receiver| async move {
                let mut receiver = receiver?;
                receiver.next().await?;
                Some((Message::MediaPoll, Some(receiver)))
            })
        })
    }
}

#[derive(Clone)]
struct Notices(Arc<Mutex<Option<mpsc::Receiver<()>>>>);
impl PartialEq for Notices {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Notices {}
impl std::hash::Hash for Notices {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&Arc::as_ptr(&self.0), state);
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Event {
    SeekDraft(f64),
    SeekCommit,
    Volume(f64),
    VolumeFocus,
}

pub(super) struct State {
    session: Option<Session>,
    pub snapshot: Snapshot,
    bitmap: Option<image::Handle>,
    seek_draft: Option<f64>,
    error: Option<String>,
}
impl State {
    fn new(session: Session) -> Self {
        // Refresh also prepares the image handle. A fast decoder may already
        // have published its paused frame before the frontend attaches.
        Self {
            snapshot: Snapshot::default(),
            session: Some(session),
            bitmap: None,
            seek_draft: None,
            error: None,
        }
    }

    pub fn refresh(&mut self) {
        let Some(session) = &self.session else {
            return;
        };
        let snapshot = session.snapshot();
        if snapshot.revision == self.snapshot.revision {
            return;
        }
        if matches!(
            snapshot.phase,
            playback::Phase::Failed(_) | playback::Phase::Stopped
        ) {
            self.bitmap = None;
            self.seek_draft = None;
        } else if snapshot.frame.as_ref().map(|f| f.serial)
            != self.snapshot.frame.as_ref().map(|f| f.serial)
        {
            self.bitmap = snapshot.frame.as_ref().map(|frame| {
                image::Handle::from_rgba(
                    frame.bitmap.width,
                    frame.bitmap.height,
                    iced::advanced::graphics::core::Bytes::from_owner(frame.bitmap.rgba.clone()),
                )
            });
        }
        self.snapshot = snapshot;
    }

    pub fn controls(&self) -> Vec<Control> {
        if !matches!(
            self.snapshot.phase,
            playback::Phase::Paused | playback::Phase::Playing | playback::Phase::Ended
        ) {
            return vec![];
        }
        let mut controls = vec![Control::Play];
        if self.snapshot.seekable && self.snapshot.duration.is_some_and(|d| !d.is_zero()) {
            controls.push(Control::Seek);
        }
        controls.extend([Control::Mute, Control::Volume]);
        controls
    }

    fn request(&mut self, command: playback::Command) {
        self.error = self
            .session
            .as_ref()
            .and_then(|session| session.request(command).err())
            .map(|error| error.to_string());
    }

    pub fn activate(&mut self, control: Control) {
        match control {
            Control::Play => self.request(playback::Command::TogglePlayback),
            Control::Mute => self.request(playback::Command::ToggleMuted),
            _ => {}
        }
    }

    pub fn event(&mut self, event: Event) -> Option<Control> {
        let control = match event {
            Event::SeekDraft(_) | Event::SeekCommit => Control::Seek,
            Event::Volume(_) | Event::VolumeFocus => Control::Volume,
        };
        if !self.controls().contains(&control) {
            return None;
        }
        match event {
            Event::SeekDraft(value) if value.is_finite() => {
                self.seek_draft = Some(value.clamp(0.0, self.duration()));
            }
            Event::SeekCommit => {
                if let Some(seconds) = self.seek_draft.take() {
                    self.request(playback::Command::Seek(Duration::from_secs_f64(seconds)));
                }
            }
            Event::Volume(value) if value.is_finite() => {
                self.request(playback::Command::Volume(value.clamp(0.0, 1.0)));
            }
            Event::VolumeFocus => {}
            _ => return None,
        }
        Some(control)
    }

    /// Focused sliders consume these keys before source-browser navigation.
    pub fn key(&mut self, control: Control, key: &keyboard::Key, shift: bool) -> bool {
        use keyboard::key::Named;
        let keyboard::Key::Named(key) = key else {
            return false;
        };
        let direction = match key {
            Named::ArrowRight | Named::ArrowUp => 1,
            Named::ArrowLeft | Named::ArrowDown => -1,
            Named::Home | Named::End => 0,
            _ => return false,
        };
        match control {
            Control::Seek => {
                let command = match key {
                    Named::Home => playback::Command::Seek(Duration::ZERO),
                    Named::End => playback::Command::Seek(Duration::from_secs_f64(self.duration())),
                    _ => {
                        let seconds = direction * if shift { 30 } else { 5 };
                        if let Some(draft) = self.seek_draft.take() {
                            playback::Command::Seek(Duration::from_secs_f64(
                                (draft + seconds as f64).clamp(0.0, self.duration()),
                            ))
                        } else {
                            playback::Command::SeekBy(seconds)
                        }
                    }
                };
                self.seek_draft = None;
                self.request(command);
            }
            Control::Volume => {
                let command = match key {
                    Named::Home => playback::Command::Volume(0.0),
                    Named::End => playback::Command::Volume(1.0),
                    _ => playback::Command::VolumeBy(direction as f64 * 0.05),
                };
                self.request(command);
            }
            _ => return false,
        }
        true
    }

    fn duration(&self) -> f64 {
        self.snapshot.duration.map_or(0.0, |d| d.as_secs_f64())
    }

    pub fn body<'a>(&'a self, desktop: &'a Desktop, video: bool) -> Element<'a, Message> {
        if let Some(bitmap) = &self.bitmap {
            return image(bitmap.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(iced::ContentFit::Contain)
                .into();
        }
        let appearance = &desktop.configuration.appearance;
        let body_size = appearance.typography().body_size.px();
        let font = canopa_ui::font(appearance);
        let notice = match &self.snapshot.phase {
            playback::Phase::Preparing => "Preparing media…",
            playback::Phase::Failed(error) => error,
            playback::Phase::Stopped => "Media preview stopped.",
            _ if video => "Preparing video frame…",
            phase => {
                let title = match phase {
                    playback::Phase::Playing => "Playing audio",
                    playback::Phase::Ended => "Playback finished",
                    playback::Phase::Paused if !self.snapshot.position.is_zero() => {
                        "Playback paused"
                    }
                    _ => "Ready to play",
                };
                let icon = container(desktop.icons.chrome(chrome::Icon::Activity, 32.0, 1.0))
                    .padding(14)
                    .style(move |_| {
                        chrome::highlight(appearance, canopa_theme::ColorRole::AccentSurface)
                    });
                return container(
                    column![icon, chrome::section_title(appearance, title)]
                        .spacing(10)
                        .align_x(iced::Alignment::Center),
                )
                .center(Length::Fill)
                .into();
            }
        };
        container(text(notice).font(font).size(body_size))
            .padding(20)
            .center(Length::Fill)
            .into()
    }

    pub fn transport<'a>(
        &'a self,
        id: window::Id,
        revision: u64,
        focused: Option<Control>,
        appearance: &'a canopa_theme::Appearance,
    ) -> Element<'a, Message> {
        let controls = self.controls();
        let body_size = appearance.typography().body_size.px();
        let label_size = appearance.typography().label_size.px();
        let font = canopa_ui::font(appearance);
        let padding = canopa_ui::content_padding(appearance.style(canopa_theme::UiKind::Button));
        let play_width =
            chrome::text_width("Pause", font, label_size) + padding.left + padding.right + 2.0;
        let button = |label, control| {
            canopa_ui::styled_action(
                text(label).font(font).size(label_size),
                controls.contains(&control).then_some(Message::Preview(
                    preview_ui::Event::Activate(id, revision, control),
                )),
                if control == Control::Play {
                    canopa_ui::Action::Normal
                } else {
                    canopa_ui::Action::Quiet
                },
                focused == Some(control),
                appearance,
            )
            .width(if control == Control::Play {
                Length::Fixed(play_width)
            } else {
                Length::Shrink
            })
        };
        let message = move |event| Message::Preview(preview_ui::Event::Media(id, revision, event));
        let position = self
            .seek_draft
            .unwrap_or(self.snapshot.position.as_secs_f64());
        let time = text(format!(
            "{} / {}{}",
            time(position as u64),
            self.snapshot
                .duration
                .map_or_else(|| "—".into(), |d| time(d.as_secs())),
            if self.snapshot.seeking {
                " · Seeking…"
            } else {
                ""
            }
        ))
        .font(font)
        .size(label_size);
        let mut transport = column![].spacing(6);
        if controls.contains(&Control::Seek) {
            transport = transport.push(
                row![
                    text("Seek").font(font).size(label_size),
                    focus_frame(
                        slider(0.0..=self.duration(), position, move |v| message(
                            Event::SeekDraft(v)
                        ))
                        .step(0.1)
                        .on_release(message(Event::SeekCommit))
                        .into(),
                        focused == Some(Control::Seek),
                    )
                    .width(Length::Fill),
                    time,
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );
        } else {
            transport = transport.push(
                row![
                    text("Seek unavailable")
                        .font(font)
                        .size(label_size)
                        .style(canopa_ui::muted)
                        .width(Length::Fill),
                    time,
                ]
                .spacing(8),
            );
        }
        let mut actions = row![
            button(
                if self.snapshot.phase == playback::Phase::Playing {
                    "Pause"
                } else {
                    "Play"
                },
                Control::Play,
            ),
            button(
                if self.snapshot.muted {
                    "Unmute"
                } else {
                    "Mute"
                },
                Control::Mute
            ),
            text(format!(
                "Volume {}%",
                (self.snapshot.volume * 100.0).round() as u32
            ))
            .font(font)
            .size(label_size),
        ]
        .spacing(7)
        .align_y(iced::Alignment::Center);
        if controls.contains(&Control::Volume) {
            actions = actions.push(
                focus_frame(
                    slider(0.0..=1.0, self.snapshot.volume, move |v| {
                        message(Event::Volume(v))
                    })
                    .step(0.05)
                    .on_release(message(Event::VolumeFocus))
                    .into(),
                    focused == Some(Control::Volume),
                )
                .width(Length::Fill)
                .max_width(220),
            );
        }
        transport = transport.push(actions);
        if let Some(notice) = self.error.as_ref().or(self.snapshot.notice.as_ref()) {
            transport = transport.push(text(notice).size(body_size));
        }
        container(transport)
            .padding(8)
            .width(Length::Fill)
            .style(move |_| chrome::highlight(appearance, canopa_theme::ColorRole::Spine))
            .into()
    }
}

fn time(seconds: u64) -> String {
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

fn focus_frame(content: Element<'_, Message>, focused: bool) -> container::Container<'_, Message> {
    container(focus::pointer_slider(content))
        .padding(4)
        .style(move |theme| container::Style {
            border: iced::Border {
                color: canopa_ui::colors(theme).focus,
                width: if focused { 2.0 } else { 0.0 },
                radius: canopa_ui::metrics::CONTROL_RADIUS.into(),
            },
            ..Default::default()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Cancellation,
        preview::{PreviewContent, PreviewRequest},
    };

    #[test]
    fn attaching_to_an_already_prepared_session_displays_its_paused_frame() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("video.webm");
        std::fs::write(
            &path,
            include_bytes!("../../tests/fixtures/preview-colors.webm"),
        )
        .unwrap();
        let PreviewContent::Video(source) = PreviewRequest::new(path)
            .prepare(&Cancellation::default())
            .unwrap()
            .content
        else {
            panic!("expected video");
        };
        let playback = Playback::silent();
        let session = playback.open(source).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        loop {
            let snapshot = session.snapshot();
            if snapshot.frame.is_some() && snapshot.phase == playback::Phase::Paused {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "video did not prepare: {:?}",
                snapshot.phase
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut state = State::new(session);
        state.refresh();
        assert_eq!(state.snapshot.phase, playback::Phase::Paused);
        assert!(state.bitmap.is_some());
        drop(state);
        while playback.holds_application() {
            assert!(std::time::Instant::now() < deadline, "video did not retire");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
