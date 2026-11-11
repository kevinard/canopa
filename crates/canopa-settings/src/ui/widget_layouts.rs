use super::{Message, State, choice, field, pick, task, toggle};
use crate::{
    session::Paths,
    storage::Snapshot,
    widget_layouts::{EDGES, Editor, Target, ZONES},
};
use canopa_session_ui::layout::{Anchor, Item, Layout, Region};
use canopa_ui::direction;
use iced::{
    Element, Length,
    widget::{column, container, responsive, row, space, stack, text},
};
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub(super) enum Action {
    Loaded(Result<Box<Editor>, String>),
    Stored(Result<Box<Editor>, String>),
    Lock(bool),
    Bar(String),
    Edge(String),
    Target(Target),
    Monitor(String),
    Output(String),
    /// An index into [`SCALES`].
    Scale(usize),
    Select(String),
    Place(String),
    Add(String),
    Shift(isize),
    Remove,
    Flow(bool),
    Hide(bool),
    Title(String),
    Duplicate,
    RemoveBar,
}

fn message(action: Action) -> Message {
    Message::WidgetLayouts(action)
}

#[derive(Default)]
pub(super) struct View {
    pub(super) editor: Option<Result<Editor, String>>,
    pub(super) lock: bool,
    bar: usize,
    output: String,
    scale: usize,
    pub(super) widget: Option<String>,
    title: String,
}

const SCALES: [f32; 4] = [1., 0.75, 1.25, 1.5];

/// A screen edge (`left`, `right`, `top`, `bottom`) in the selected language.
fn edge_label(edge: &str) -> String {
    match edge {
        "left" => fl!("settings-widgets-edge-left"),
        "right" => fl!("settings-widgets-edge-right"),
        "top" => fl!("settings-widgets-edge-top"),
        _ => fl!("settings-widgets-edge-bottom"),
    }
}

/// A bar zone (`start`, `center`, `end`) or lock-screen region in the
/// selected language.
fn place_label(place: &str) -> String {
    match place {
        "start" => fl!("settings-widgets-zone-start"),
        "center" => fl!("settings-widgets-zone-center"),
        "end" => fl!("settings-widgets-zone-end"),
        "top-left" => fl!("settings-widgets-region-top-left"),
        "top-center" => fl!("settings-widgets-region-top-center"),
        "top-right" => fl!("settings-widgets-region-top-right"),
        "center-left" => fl!("settings-widgets-region-center-left"),
        "center-right" => fl!("settings-widgets-region-center-right"),
        "bottom-left" => fl!("settings-widgets-region-bottom-left"),
        "bottom-center" => fl!("settings-widgets-region-bottom-center"),
        "bottom-right" => fl!("settings-widgets-region-bottom-right"),
        other => other.to_owned(),
    }
}

impl View {
    pub(super) fn load(paths: &Paths) -> iced::Task<Message> {
        let path = paths.common();
        task(
            move || Editor::load(Snapshot::load(path)?).map(Box::new),
            |result| message(Action::Loaded(result)),
        )
    }

    pub(super) fn dirty(&self) -> bool {
        self.editor
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .is_some_and(Editor::dirty)
    }

    fn selected(&self) -> Option<&str> {
        self.widget.as_deref()
    }
}

impl State {
    pub(super) fn save_widget_layouts(&mut self) -> iced::Task<Message> {
        let Some(editor) = self
            .widget_layouts
            .editor
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .filter(|editor| editor.dirty())
        else {
            self.busy = false;
            return iced::Task::none();
        };
        let editor = editor.clone();
        task(
            move || Editor::load(editor.save()?).map(Box::new),
            |result| message(Action::Stored(result)),
        )
    }

    pub(super) fn update_widget_layouts(&mut self, action: Action) {
        let view = &mut self.widget_layouts;
        match action {
            Action::Loaded(result) => {
                if view.editor.is_none() {
                    view.editor = Some(result.map(|editor| *editor));
                }
                return;
            }
            Action::Stored(result) => {
                self.busy = false;
                match result {
                    Ok(editor) => {
                        // The shared-document rule: Personalization keeps its
                        // draft only if it saw the version this save replaced.
                        if let Some(previous) = view
                            .editor
                            .as_ref()
                            .and_then(|result| result.as_ref().ok())
                            .map(|editor| editor.source.clone())
                        {
                            self.document_saved(
                                crate::Page::Widgets,
                                crate::registry::Document::Common,
                                Some((&previous, &editor.source)),
                            );
                        }
                        let view = &mut self.widget_layouts;
                        view.bar = view.bar.min(editor.bars.len() - 1);
                        view.editor = Some(Ok(*editor));
                        self.notice = fl!("settings-widgets-saved");
                    }
                    Err(error) => self.notice = error,
                }
                return;
            }
            _ if self.busy || self.trial.is_some() => return,
            _ => {}
        }
        let Some(Ok(editor)) = &mut view.editor else {
            return;
        };
        let selected = view.widget.as_deref();
        match action {
            Action::Loaded(_) | Action::Stored(_) => unreachable!(),
            Action::Lock(lock) => {
                view.lock = lock;
                view.widget = None;
            }
            Action::Bar(name) => {
                if let Some(index) = editor.bars.iter().position(|bar| bar.name == name) {
                    view.bar = index;
                    view.widget = None;
                }
            }
            Action::Scale(index) => {
                if index < SCALES.len() {
                    view.scale = index;
                }
            }
            Action::Output(output) => view.output = output,
            Action::Select(name) => view.widget = Some(name),
            Action::Title(title) => view.title = title.chars().take(80).collect(),
            Action::Duplicate => match editor.duplicate_bar(view.bar, view.title.trim()) {
                Ok(index) => {
                    view.bar = index;
                    view.title.clear();
                    self.notice.clear();
                }
                Err(error) => self.notice = error.to_string(),
            },
            Action::RemoveBar => {
                editor.remove_bar(view.bar);
                view.bar = view.bar.min(editor.bars.len() - 1);
                view.widget = None;
            }
            Action::Edge(edge) => {
                let bar = &mut editor.bars[view.bar];
                bar.edge = edge;
                bar.change();
            }
            Action::Target(target) => {
                let bar = &mut editor.bars[view.bar];
                bar.target = target;
                bar.change();
            }
            Action::Monitor(output) => {
                let bar = &mut editor.bars[view.bar];
                if !bar.outputs.remove(&output) {
                    bar.outputs.insert(output);
                }
                bar.change();
            }
            Action::Hide(hide) => {
                if let Some(name) = selected {
                    editor.hide_lock_widget(&view.output, name, hide);
                }
            }
            Action::Place(zone) => {
                if let Some(name) = selected {
                    if view.lock {
                        if let Some(anchor) =
                            Anchor::ALL.into_iter().find(|anchor| anchor.key() == zone)
                        {
                            editor.place_lock(name, Some(anchor));
                        }
                    } else if let Some(index) = ZONES.iter().position(|key| *key == zone) {
                        editor.bars[view.bar].place(name, Some(index));
                    }
                }
            }
            Action::Add(name) => {
                if view.lock {
                    let anchor = selected
                        .and_then(|name| lock_region(editor, name))
                        .map_or(Anchor::BottomCenter, |region| region.anchor);
                    editor.place_lock(&name, Some(anchor));
                } else {
                    let bar = &mut editor.bars[view.bar];
                    let zone = selected.and_then(|name| bar_zone(bar, name)).unwrap_or(2);
                    bar.place(&name, Some(zone));
                }
                view.widget = Some(name);
            }
            Action::Remove => {
                if let Some(name) = selected {
                    if view.lock {
                        editor.place_lock(name, None);
                    } else {
                        editor.bars[view.bar].place(name, None);
                    }
                    view.widget = None;
                }
            }
            Action::Flow(row) => {
                if let Some(region) = selected.and_then(|name| lock_region(editor, name)) {
                    editor.region(region.anchor).row = row;
                    editor.change_lock();
                }
            }
            Action::Shift(direction) => {
                if let Some(name) = selected {
                    if view.lock {
                        if let Some(region) = lock_region(editor, name) {
                            let anchor = region.anchor;
                            let active = region
                                .widgets
                                .iter()
                                .filter(|name| editor.lock_active(name))
                                .cloned()
                                .collect::<Vec<_>>();
                            if let Some(other) = adjacent(&active, name, direction) {
                                let widgets = &mut editor.region(anchor).widgets;
                                swap(widgets, name, other);
                                editor.change_lock();
                            }
                        }
                    } else {
                        let bar = &mut editor.bars[view.bar];
                        if let Some(zone) = bar_zone(bar, name) {
                            let widgets = &mut bar.zones[zone];
                            if let Some(other) =
                                adjacent(widgets, name, direction).map(str::to_owned)
                            {
                                swap(widgets, name, &other);
                                bar.change();
                            }
                        }
                    }
                }
            }
        }
    }

    pub(super) fn widget_layouts_view(&self) -> Element<'_, Message> {
        let view = &self.widget_layouts;
        let Some(result) = &view.editor else {
            return text(fl!("settings-widgets-loading")).into();
        };
        let editor = match result {
            Ok(editor) => editor,
            Err(error) => return text(error.as_str()).into(),
        };
        let mut content = column![
            direction::row([
                self.action(
                    "settings-button-bar-layout",
                    fl!("settings-widgets-bar-layout"),
                    Some(message(Action::Lock(false))),
                    !view.lock,
                ),
                self.action(
                    "settings-button-lock-screen-layout",
                    fl!("settings-widgets-lock-layout"),
                    Some(message(Action::Lock(true))),
                    view.lock,
                ),
                self.action(
                    "settings-button-refresh-monitors",
                    fl!("settings-widgets-refresh"),
                    (!self.refreshing).then_some(Message::BackendChanged),
                    false,
                ),
            ])
            .spacing(8)
        ]
        .spacing(12);

        let bar = &editor.bars[view.bar];
        let mut outputs = self.outputs.keys().cloned().collect::<BTreeSet<_>>();
        if view.lock {
            outputs.extend(editor.lock.outputs.keys().cloned());
        } else {
            outputs.extend(bar.outputs.iter().cloned());
            content = content.push(
                direction::row([
                    column![
                        text(fl!("settings-widgets-bar")),
                        choice(
                            "widgets-bar",
                            editor.bars.iter().map(|bar| bar.name.clone()).collect(),
                            Some(bar.name.clone()),
                            |name| message(Action::Bar(name))
                        )
                    ]
                    .spacing(4)
                    .width(Length::Fill)
                    .into(),
                    column![
                        text(fl!("settings-widgets-edge")),
                        pick(
                            "widgets-edge",
                            EDGES
                                .iter()
                                .map(|edge| ((*edge).to_owned(), edge_label(edge)))
                                .collect(),
                            Some(bar.edge.clone()),
                            |edge| message(Action::Edge(edge))
                        )
                    ]
                    .spacing(4)
                    .width(Length::Fill)
                    .into(),
                ])
                .spacing(12),
            );
        }
        if !view.output.is_empty() {
            outputs.insert(view.output.clone());
        }
        // "" is every monitor; others by connector name.
        let preview_outputs =
            std::iter::once((String::new(), fl!("settings-widgets-all-monitors")))
                .chain(
                    outputs
                        .iter()
                        .map(|output| (output.clone(), output.clone())),
                )
                .collect();
        content = content.push(
            direction::row([
                column![
                    text(fl!("settings-widgets-preview-monitor")),
                    pick(
                        "widgets-output",
                        preview_outputs,
                        Some(view.output.clone()),
                        |output| message(Action::Output(output))
                    )
                ]
                .spacing(4)
                .width(Length::Fill)
                .into(),
                column![
                    text(fl!("settings-widgets-preview-scale")),
                    pick(
                        "widgets-scale",
                        SCALES
                            .iter()
                            .enumerate()
                            .map(|(index, scale)| {
                                (
                                    index,
                                    canopa_i18n::format::percent(f64::from(*scale) * 100.),
                                )
                            })
                            .collect(),
                        Some(view.scale),
                        |scale| message(Action::Scale(scale))
                    )
                ]
                .spacing(4)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(12),
        );
        content = content.push(text(fl!("settings-widgets-preview-hint")).size(14));
        content = content.push(self.widget_preview(editor));
        content = content.push(self.placement_controls(editor));
        if !view.lock {
            content = content.push(text(fl!("settings-widgets-visibility")).size(20));
            content = content.push(pick(
                "widgets-target",
                Target::ALL
                    .map(|target| {
                        let label = target.label();
                        (target, label)
                    })
                    .to_vec(),
                Some(bar.target.clone()),
                |target| message(Action::Target(target)),
            ));
            if bar.target != Target::All {
                for output in outputs {
                    content = content.push(toggle(
                        format!("widgets-monitor-{output}"),
                        &output,
                        bar.outputs.contains(&output),
                        message(Action::Monitor(output.clone())),
                    ));
                }
                if bar.outputs.is_empty() {
                    content = content.push(text(fl!("settings-widgets-choose-monitor")));
                }
            }
            content = content.push(field(
                "widgets-copy-name",
                fl!("settings-widgets-copy-name"),
                &view.title,
                |title| message(Action::Title(title)),
            ));
            content = content.push(
                direction::row([
                    self.action(
                        "settings-button-duplicate-bar",
                        fl!("settings-widgets-duplicate-bar"),
                        (!view.title.trim().is_empty()).then_some(message(Action::Duplicate)),
                        false,
                    ),
                    self.action(
                        "settings-button-remove-bar",
                        fl!("settings-widgets-remove-bar"),
                        (editor.bars.len() > 1).then_some(message(Action::RemoveBar)),
                        false,
                    ),
                ])
                .spacing(8),
            );
        } else {
            content = content.push(text(fl!("settings-widgets-lock-hint")).size(14));
        }
        content.into()
    }

    fn placement_controls(&self, editor: &Editor) -> Element<'_, Message> {
        let view = &self.widget_layouts;
        let bar = &editor.bars[view.bar];
        let placed: Vec<String> = if view.lock {
            editor
                .lock
                .layout
                .regions
                .iter()
                .flat_map(|region| &region.widgets)
                .filter(|name| editor.lock_active(name))
                .cloned()
                .collect()
        } else {
            bar.zones.iter().flatten().cloned().collect()
        };
        let available = if view.lock {
            editor
                .lock_catalog()
                .into_iter()
                .filter(|name| !placed.contains(name))
                .collect()
        } else {
            editor
                .modules
                .iter()
                .filter(|name| !placed.contains(name))
                .cloned()
                .collect()
        };
        let selected = view
            .selected()
            .filter(|name| placed.iter().any(|widget| widget == name));
        let mut controls = column![
            direction::row([
                column![
                    text(fl!("settings-widgets-placed")),
                    choice(
                        "widgets-selection",
                        placed,
                        selected.map(str::to_owned),
                        |name| message(Action::Select(name))
                    )
                ]
                .spacing(4)
                .width(Length::Fill)
                .into(),
                column![
                    text(fl!("settings-widgets-add")),
                    choice("widgets-add", available, None, |name| message(Action::Add(
                        name
                    )))
                ]
                .spacing(4)
                .width(Length::Fill)
                .into(),
            ])
            .spacing(12)
        ]
        .spacing(10);
        if let Some(name) = selected {
            let zone;
            let options;
            let siblings;
            if view.lock {
                let region = lock_region(editor, name).expect("placed widget has a region");
                zone = region.anchor.key();
                options = Anchor::ALL.map(|anchor| anchor.key().to_owned()).to_vec();
                siblings = region
                    .widgets
                    .iter()
                    .filter(|name| editor.lock_active(name))
                    .cloned()
                    .collect::<Vec<_>>();
            } else {
                let index = bar_zone(bar, name).expect("placed widget has a bar zone");
                zone = ZONES[index];
                options = ZONES.map(String::from).to_vec();
                siblings = bar.zones[index].clone();
            }
            controls = controls.push(
                direction::row([
                    column![
                        text(if view.lock {
                            fl!("settings-widgets-region")
                        } else {
                            fl!("settings-widgets-zone")
                        }),
                        pick(
                            "widgets-placement",
                            options
                                .into_iter()
                                .map(|place| {
                                    let label = place_label(&place);
                                    (place, label)
                                })
                                .collect(),
                            Some(zone.to_owned()),
                            |zone| message(Action::Place(zone))
                        )
                    ]
                    .spacing(4)
                    .width(Length::Fill)
                    .into(),
                    self.action(
                        "settings-button-earlier",
                        fl!("settings-widgets-earlier"),
                        adjacent(&siblings, name, -1)
                            .is_some()
                            .then_some(message(Action::Shift(-1))),
                        false,
                    ),
                    self.action(
                        "settings-button-later",
                        fl!("settings-widgets-later"),
                        adjacent(&siblings, name, 1)
                            .is_some()
                            .then_some(message(Action::Shift(1))),
                        false,
                    ),
                    self.action(
                        "settings-button-remove-widget",
                        fl!("settings-widgets-remove"),
                        (name != "authentication").then_some(message(Action::Remove)),
                        false,
                    ),
                ])
                .spacing(8)
                .align_y(iced::Alignment::End),
            );
            if view.lock {
                let region = lock_region(editor, name).unwrap();
                controls = controls.push(toggle(
                    "widgets-region-flow",
                    fl!("settings-widgets-flow"),
                    region.row,
                    message(Action::Flow(!region.row)),
                ));
                if !view.output.is_empty() && name != "authentication" {
                    let hidden = editor
                        .lock
                        .hidden_on(&view.output)
                        .iter()
                        .any(|widget| widget == name);
                    controls = controls.push(toggle(
                        "widgets-hide",
                        fl!("settings-widgets-hide"),
                        hidden,
                        message(Action::Hide(!hidden)),
                    ));
                }
            }
        }
        controls.into()
    }

    fn widget_preview<'a>(&'a self, editor: &'a Editor) -> Element<'a, Message> {
        responsive(move |size| self.draw_widget_preview(editor, size.width))
            .height(Length::Shrink)
            .into()
    }

    fn draw_widget_preview(&self, editor: &Editor, available_width: f32) -> Element<'_, Message> {
        let view = &self.widget_layouts;
        let (width, height) = preview_size(self.outputs.get(&view.output), available_width);
        let bar = &editor.bars[view.bar];
        let layout;
        let visible: Vec<&str>;
        if view.lock {
            layout = editor.lock.layout.clone();
            visible = layout
                .regions
                .iter()
                .flat_map(|region| &region.widgets)
                .filter(|name| {
                    editor.lock_active(name) && !editor.lock.hidden_on(&view.output).contains(name)
                })
                .map(String::as_str)
                .collect();
        } else {
            if !view.output.is_empty() && !bar.visible_on(&view.output) {
                return container(text(fl!("settings-widgets-bar-hidden")))
                    .width(width)
                    .height(height)
                    .center_x(width)
                    .center_y(height)
                    .style(canopa_ui::surface)
                    .into();
            }
            layout = bar_preview_layout(bar);
            visible = bar.zones.iter().flatten().map(String::as_str).collect();
        }
        let scale = SCALES[view.scale];
        let items = visible
            .iter()
            .map(|name| {
                let label = preview_label(name, view.lock);
                let (width, height) = match (*name, view.lock) {
                    ("authentication", true) => (138., 34.),
                    ("clock", true) => (68., 27.),
                    _ => (
                        (label.chars().count() as f32 * 6. + 16.).clamp(36., 118.),
                        22.,
                    ),
                };
                Item {
                    name: (*name).into(),
                    width: width * scale,
                    height: height * scale,
                }
            })
            .collect::<Vec<_>>();
        let geometry_scale = if view.lock {
            width / 1280. * scale
        } else {
            scale
        };
        let mut preview = stack![
            container(space())
                .width(width)
                .height(height)
                .style(canopa_ui::surface)
        ];
        if !view.lock {
            let thickness = if matches!(bar.edge.as_str(), "top" | "bottom") {
                30. * scale
            } else {
                items
                    .iter()
                    .map(|item| item.width)
                    .fold(48. * scale, f32::max)
                    + 8. * scale
            };
            let (x, y, w, h) = match bar.edge.as_str() {
                "right" => (width - thickness, 0., thickness, height),
                "top" => (0., 0., width, thickness),
                "bottom" => (0., height - thickness, width, thickness),
                _ => (0., 0., thickness, height),
            };
            preview = preview.push(positioned(
                x,
                y,
                container(space())
                    .width(w)
                    .height(h)
                    .style(container::rounded_box)
                    .into(),
            ));
        }
        for placement in layout.plan(width, height, geometry_scale, &items) {
            let bounds = placement.bounds;
            let active = view.selected() == Some(placement.name.as_str());
            let label = preview_label(&placement.name, view.lock);
            let content = container(
                text(label.clone())
                    .size((12. * scale * placement.scale).max(8.))
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .center_x(Length::Fill)
            .center_y(Length::Fill);
            let select = message(Action::Select(placement.name.clone()));
            let button = canopa_ui::action(
                content,
                Some(select.clone()),
                canopa_ui::Action::Normal,
                active,
            )
            .padding(0)
            .width(bounds.width)
            .height(bounds.height);
            let button = canopa_ui::accessibility::semantic(
                canopa_ui::accessibility::Node::button(label.clone())
                    .key(format!("widget-preview-{}", placement.name))
                    .toggled(active)
                    .on_click(Some(select)),
                button,
            );
            preview = preview.push(positioned(bounds.x, bounds.y, button));
        }
        preview.width(width).height(height).into()
    }
}

fn preview_size(output: Option<&niri_ipc::Output>, available_width: f32) -> (f32, f32) {
    let ratio = output
        .and_then(|output| output.logical)
        .map_or(16. / 9., |mode| {
            (mode.width as f32 / mode.height.max(1) as f32).clamp(0.5, 3.)
        });
    let height = (available_width.clamp(1., 420.) / ratio).min(252.);
    (height * ratio, height)
}

fn preview_label(name: &str, lock: bool) -> String {
    use canopa_i18n::format;
    let sample = if lock {
        match name {
            "clock" => format::time("12:45:00".parse().expect("a valid time")),
            "date" => format::weekday_month_day("2026-11-03".parse().expect("a valid date")),
            "authentication" => fl!("settings-widgets-sample-password"),
            "username" => fl!("settings-widgets-sample-user"),
            "battery" => format::percent(82.),
            "media" => fl!("settings-widgets-sample-media"),
            "weather" => fl!("settings-widgets-sample-weather"),
            "keyboard" => fl!("settings-widgets-sample-keyboard"),
            "avatar" => fl!("settings-widgets-sample-avatar"),
            _ => name.to_owned(),
        }
    } else {
        name.to_owned()
    };
    sample.chars().take(19).collect()
}

fn positioned<'a>(x: f32, y: f32, element: Element<'a, Message>) -> Element<'a, Message> {
    column![space().height(y), row![space().width(x), element]].into()
}

fn bar_preview_layout(bar: &crate::widget_layouts::Bar) -> Layout {
    let (anchors, row) = match bar.edge.as_str() {
        "top" => ([Anchor::TopLeft, Anchor::TopCenter, Anchor::TopRight], true),
        "bottom" => (
            [
                Anchor::BottomLeft,
                Anchor::BottomCenter,
                Anchor::BottomRight,
            ],
            true,
        ),
        "right" => (
            [Anchor::TopRight, Anchor::CenterRight, Anchor::BottomRight],
            false,
        ),
        _ => (
            [Anchor::TopLeft, Anchor::CenterLeft, Anchor::BottomLeft],
            false,
        ),
    };
    Layout {
        margin: 4.,
        gap: 4.,
        regions: anchors
            .into_iter()
            .zip(&bar.zones)
            .map(|(anchor, widgets)| Region {
                anchor,
                widgets: widgets.clone(),
                row,
                x: 0.,
                y: 0.,
            })
            .collect(),
    }
}

fn bar_zone(bar: &crate::widget_layouts::Bar, name: &str) -> Option<usize> {
    bar.zones
        .iter()
        .position(|zone| zone.iter().any(|widget| widget == name))
}

fn lock_region<'a>(editor: &'a Editor, name: &str) -> Option<&'a Region> {
    editor
        .lock
        .layout
        .regions
        .iter()
        .find(|region| region.widgets.iter().any(|widget| widget == name))
}

fn adjacent<'a>(widgets: &'a [String], name: &str, direction: isize) -> Option<&'a str> {
    let position = widgets.iter().position(|widget| widget == name)?;
    widgets
        .get(position.checked_add_signed(direction)?)
        .map(String::as_str)
}

fn swap(widgets: &mut [String], name: &str, other: &str) {
    let left = widgets.iter().position(|widget| widget == name).unwrap();
    let right = widgets.iter().position(|widget| widget == other).unwrap();
    widgets.swap(left, right);
}
