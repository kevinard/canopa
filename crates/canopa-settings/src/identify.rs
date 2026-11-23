//! Labels on the displays themselves while Displays arranges them: each
//! screen shows its number and name in a corner, and the one being dragged
//! is lit. Settings runs this as a child process (`identify-displays`) and
//! writes the labels to its standard input, one JSON line per change; the
//! labels go away when that input closes.
use crate::{file_page::FileDraft, preferences::Personalization, session::Paths};
use iced::{
    Alignment, Element, Length, Subscription, Task, Theme,
    font::Weight,
    widget::{container, row, text},
    window::Id,
};
use iced_layershell::{
    actions::{LayerShellCustomAction, LayerShellCustomActionWithId},
    reexport::{Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings, OutputOption},
    settings::{LayerShellSettings, Settings, StartMode},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// One display's label.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Label {
    /// The connector the label shows on.
    pub output: String,
    pub number: usize,
    pub name: String,
    /// Whether this display is the one being moved.
    pub lit: bool,
}

/// Every label at once, as Settings writes them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Labels {
    pub displays: Vec<Label>,
}

/// The label's size. Its surface is drawn opaque, so the label fills it,
/// square, with nothing transparent around it.
const WIDTH: u32 = 340;
const HEIGHT: u32 = 64;

#[derive(Debug, Clone)]
enum Message {
    Labels(Vec<Label>),
    Closed,
    Outputs,
    Open {
        settings: NewLayerShellSettings,
        id: Id,
    },
    Remove(Id),
}

impl TryInto<LayerShellCustomActionWithId> for Message {
    type Error = Self;
    fn try_into(self) -> Result<LayerShellCustomActionWithId, Self> {
        match self {
            Self::Open { settings, id } => Ok(LayerShellCustomActionWithId(
                None,
                LayerShellCustomAction::NewLayerShell { settings, id },
            )),
            Self::Remove(id) => Ok(LayerShellCustomActionWithId(
                Some(id),
                LayerShellCustomAction::RemoveWindow,
            )),
            message => Err(message),
        }
    }
}

impl From<iced_layershell::WaylandOutputSnapshot> for Message {
    fn from(_: iced_layershell::WaylandOutputSnapshot) -> Self {
        Self::Outputs
    }
}

struct Identify {
    windows: HashMap<String, (Id, Label)>,
    font: iced::Font,
}

impl Identify {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Labels(labels) => {
                let mut tasks = Vec::new();
                let wanted: HashMap<_, _> = labels
                    .into_iter()
                    .take(32)
                    .map(|label| (label.output.clone(), label))
                    .collect();
                self.windows.retain(|output, (id, _)| {
                    let keep = wanted.contains_key(output);
                    if !keep {
                        tasks.push(Task::done(Message::Remove(*id)));
                    }
                    keep
                });
                for (output, label) in wanted {
                    match self.windows.get_mut(&output) {
                        Some((_, current)) => *current = label,
                        None => {
                            let id = Id::unique();
                            tasks.push(Task::done(Message::Open {
                                settings: surface(&output),
                                id,
                            }));
                            self.windows.insert(output, (id, label));
                        }
                    }
                }
                Task::batch(tasks)
            }
            Message::Closed => iced::exit(),
            Message::Outputs | Message::Open { .. } | Message::Remove(_) => Task::none(),
        }
    }

    fn view(&self, window: Id) -> Element<'_, Message> {
        let Some((_, label)) = self.windows.values().find(|(id, _)| *id == window) else {
            return iced::widget::space().into();
        };
        let lit = label.lit;
        let font = self.font;
        let number = container(
            text(label.number.to_string())
                .size(22)
                .font(iced::Font {
                    weight: Weight::Bold,
                    ..font
                })
                .style(move |theme: &Theme| {
                    let p = canopa_ui::colors(theme);
                    text::Style {
                        color: Some(if lit { p.accent } else { p.on_accent }),
                    }
                }),
        )
        .center_x(40)
        .center_y(40)
        .style(move |theme: &Theme| {
            let p = canopa_ui::colors(theme);
            container::Style {
                background: Some(if lit { p.on_accent } else { p.accent }.into()),
                border: iced::Border {
                    radius: 20.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        });
        let label = container(
            row![
                number,
                text(label.name.clone())
                    .size(17)
                    .font(iced::Font {
                        weight: Weight::Semibold,
                        ..font
                    })
                    .wrapping(text::Wrapping::None)
                    .style(move |theme: &Theme| {
                        let p = canopa_ui::colors(theme);
                        text::Style {
                            color: Some(if lit { p.on_accent } else { p.text }),
                        }
                    }),
            ]
            .spacing(14)
            .align_y(Alignment::Center),
        )
        .padding([12, 14])
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .style(move |theme: &Theme| {
            let p = canopa_ui::colors(theme);
            container::Style {
                background: Some(if lit { p.accent } else { p.raised }.into()),
                border: iced::Border {
                    color: p.border,
                    width: if lit { 0.0 } else { 1.0 },
                    radius: 0.0.into(),
                },
                ..Default::default()
            }
        });
        label.into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::run(read_labels)
    }
}

/// A label in the top left corner of `output`, clear of reserved areas such
/// as the bar, that never takes input.
fn surface(output: &str) -> NewLayerShellSettings {
    NewLayerShellSettings {
        size: Some((WIDTH, HEIGHT)),
        layer: Layer::Overlay,
        anchor: Anchor::Top | Anchor::Left,
        exclusive_zone: Some(0),
        margin: Some((24, 0, 0, 24)),
        keyboard_interactivity: KeyboardInteractivity::None,
        output_option: OutputOption::OutputName(output.into()),
        events_transparent: true,
        namespace: Some("canopa-settings-identify".into()),
    }
}

/// The labels from standard input, then `Closed` when it ends.
fn read_labels() -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(8, async |mut output| {
        use iced::futures::{SinkExt, StreamExt};
        let (send, mut receive) = iced::futures::channel::mpsc::unbounded();
        std::thread::spawn(move || {
            use std::io::BufRead;
            for line in std::io::stdin().lock().lines() {
                let Ok(line) = line else { break };
                if let Ok(labels) = serde_json::from_str::<Labels>(&line)
                    && send
                        .unbounded_send(Message::Labels(labels.displays))
                        .is_err()
                {
                    return;
                }
            }
            let _ = send.unbounded_send(Message::Closed);
        });
        while let Some(message) = receive.next().await {
            if output.send(message).await.is_err() {
                break;
            }
        }
    })
}

/// Show labels until standard input closes.
pub fn run() -> anyhow::Result<()> {
    let appearance = Paths::discover()
        .map(|paths| {
            FileDraft::<Personalization>::load(&paths)
                .0
                .value
                .values
                .appearance
        })
        .unwrap_or_default();
    let font = canopa_ui::font(&appearance);
    let theme = canopa_ui::theme(appearance);
    iced_layershell::build_pattern::daemon(
        move || {
            (
                Identify {
                    windows: HashMap::new(),
                    font,
                },
                Task::none(),
            )
        },
        "canopa-settings-identify",
        Identify::update,
        Identify::view,
    )
    .settings(Settings {
        default_font: font,
        layer_settings: LayerShellSettings {
            start_mode: StartMode::Background,
            ..Default::default()
        },
        ..Default::default()
    })
    .theme(move |_: &Identify, _| theme.clone())
    .style(|_: &Identify, theme: &Theme| iced::theme::Style {
        background_color: iced::Color::TRANSPARENT,
        text_color: canopa_ui::colors(theme).text,
    })
    .subscription(Identify::subscription)
    .run()?;
    Ok(())
}
