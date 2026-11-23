use super::{
    Message, State, choice, hinted_choice, kit, noted_group, pick, settings_group, task, toggle,
};
use crate::{
    session::Paths,
    storage::Snapshot,
    widget_layouts::{EDGES, Editor, Target, ZONES},
};
use canopa_session_ui::layout::{Anchor, Placement, Rect, Region};
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
    /// Select a widget, then act on it.
    For(String, Box<Action>),
    /// Pick up a widget in the preview (and select it).
    Grab(String),
    /// The pointer moved over the preview.
    Pointer(iced::Point),
    /// The pointer was released over the preview, drawn in this much width.
    Release(f32),
    /// The pointer left the preview: a drag ends where it started.
    Leave,
}

/// A widget picked up in the preview.
#[derive(Clone, Debug)]
pub(super) struct Drag {
    name: String,
    /// Where the pointer is over the preview, once it moved.
    pointer: Option<iced::Point>,
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
    pub(super) widget: Option<String>,
    title: String,
    drag: Option<Drag>,
}

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
/// A bar zone named by where it sits on a bar along `edge`: top, middle
/// and bottom on a side bar; left, center and right on a top or bottom bar.
/// Lock screen regions keep their own names.
fn zone_label(place: &str, edge: Option<&str>) -> String {
    let vertical = matches!(edge, Some("left" | "right"));
    match (place, edge.is_some(), vertical) {
        ("start", true, true) => fl!("settings-widgets-zone-top"),
        ("center", true, true) => fl!("settings-widgets-zone-middle"),
        ("end", true, true) => fl!("settings-widgets-zone-bottom"),
        ("start", true, false) => fl!("settings-widgets-zone-left"),
        ("end", true, false) => fl!("settings-widgets-zone-right"),
        _ => place_label(place),
    }
}

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
        if let Action::For(name, action) = action {
            self.widget_layouts.widget = Some(name);
            return self.update_widget_layouts(*action);
        }
        let view = &mut self.widget_layouts;
        match action {
            Action::Loaded(result) => {
                if view.editor.is_none() {
                    view.editor = Some(result.map(|editor| *editor));
                    // Start with a widget chosen, so its placement controls show.
                    if view.widget.is_none()
                        && let Some(Ok(editor)) = &view.editor
                    {
                        view.widget = editor
                            .bars
                            .get(view.bar)
                            .and_then(|bar| bar.zones.iter().flatten().next().cloned());
                    }
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
            Action::Loaded(_) | Action::Stored(_) | Action::For(..) => unreachable!(),
            Action::Grab(name) => {
                view.widget = Some(name.clone());
                view.drag = Some(Drag {
                    name,
                    pointer: None,
                });
            }
            Action::Pointer(point) => {
                if let Some(drag) = &mut view.drag {
                    drag.pointer = Some(point);
                }
            }
            Action::Leave => view.drag = None,
            Action::Release(width) => {
                // A press without a move only selected the widget.
                if let Some(Drag {
                    name,
                    pointer: Some(point),
                }) = view.drag.take()
                {
                    let geometry = Geometry::new(
                        editor,
                        &Sight {
                            lock: view.lock,
                            bar: view.bar,
                            output: &view.output,
                        },
                        width,
                    );
                    if let Some(drop) = geometry.drop_at(point, &name) {
                        let before = drop.before.as_deref();
                        if view.lock {
                            if let Some(anchor) = Anchor::ALL
                                .into_iter()
                                .find(|anchor| anchor.key() == drop.place)
                            {
                                editor.place_lock_before(&name, Some(anchor), before);
                            }
                        } else if let Some(zone) = ZONES.iter().position(|zone| *zone == drop.place)
                        {
                            editor.bars[view.bar].place_before(&name, Some(zone), before);
                        }
                    }
                }
            }
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
            return kit::loading(fl!("settings-widgets-loading"));
        };
        let editor = match result {
            Ok(editor) => editor,
            Err(error) => {
                return kit::status_card(
                    kit::Notice::Warning,
                    fl!("settings-widgets-unavailable"),
                    Some(error.clone()),
                );
            }
        };
        // Which layout is edited, then the page's one other action.
        let mut content = column![
            direction::row([kit::segmented([
                self.segment(
                    "settings-button-bar-layout",
                    fl!("settings-widgets-bar-tab"),
                    view.lock.then_some(message(Action::Lock(false))),
                    !view.lock,
                ),
                self.segment(
                    "settings-button-lock-screen-layout",
                    fl!("settings-widgets-lock-tab"),
                    (!view.lock).then_some(message(Action::Lock(true))),
                    view.lock,
                ),
            ]),])
            .align_y(iced::Alignment::Center)
        ]
        .spacing(28);

        // The preview is the editor: drag to move, select for the rest.
        content = content.push(settings_group(
            fl!("settings-widgets-arrange"),
            column![
                kit::caption(fl!("settings-widgets-drag-hint")),
                self.widget_preview(editor),
                self.selected_widget(editor),
            ]
            .spacing(14),
        ));

        let bar = &editor.bars[view.bar];
        let mut outputs = self.outputs.keys().cloned().collect::<BTreeSet<_>>();
        if view.lock {
            outputs.extend(editor.lock.outputs.keys().cloned());
        } else {
            outputs.extend(bar.outputs.iter().cloned());
        }
        if !view.output.is_empty() {
            outputs.insert(view.output.clone());
        }
        if !view.lock {
            let mut bar_rows: Vec<Element<'_, Message>> = Vec::new();
            if editor.bars.len() > 1 {
                bar_rows.push(kit::row(
                    fl!("settings-widgets-bar"),
                    None,
                    choice(
                        "widgets-bar",
                        editor.bars.iter().map(|bar| bar.name.clone()).collect(),
                        Some(bar.name.clone()),
                        |name| message(Action::Bar(name)),
                    ),
                ));
            }
            bar_rows.push(kit::row(
                fl!("settings-widgets-edge"),
                None,
                pick(
                    "widgets-edge",
                    EDGES
                        .iter()
                        .map(|edge| ((*edge).to_owned(), edge_label(edge)))
                        .collect(),
                    Some(bar.edge.clone()),
                    |edge| message(Action::Edge(edge)),
                ),
            ));
            bar_rows.push(kit::row(
                fl!("settings-widgets-shown-on"),
                None,
                pick(
                    "widgets-target",
                    Target::ALL
                        .map(|target| {
                            let label = target.label();
                            (target, label)
                        })
                        .to_vec(),
                    Some(bar.target.clone()),
                    |target| message(Action::Target(target)),
                ),
            ));
            if bar.target != Target::All {
                for output in &outputs {
                    bar_rows.push(super::described_toggle(
                        format!("widgets-monitor-{output}"),
                        self.monitor_name(output),
                        None,
                        bar.outputs.contains(output),
                        message(Action::Monitor(output.clone())),
                    ));
                }
                if bar.outputs.is_empty() {
                    bar_rows.push(kit::caption(fl!("settings-widgets-choose-monitor")));
                }
            }
            content = content.push(settings_group(
                fl!("settings-widgets-bar-tab"),
                kit::rows(bar_rows),
            ));
            content = content.push(settings_group(
                fl!("settings-widgets-more-bars"),
                kit::rows([
                    kit::row(
                        fl!("settings-widgets-duplicate-bar"),
                        Some(fl!("settings-widgets-duplicate-hint")),
                        direction::row([
                            super::bare_field(
                                "widgets-copy-name",
                                fl!("settings-widgets-copy-name"),
                                &fl!("settings-widgets-copy-name"),
                                &view.title,
                                |title| message(Action::Title(title)),
                                Length::Fixed(180.),
                            ),
                            self.action(
                                "settings-button-duplicate-bar",
                                fl!("settings-widgets-duplicate-short"),
                                (!view.title.trim().is_empty())
                                    .then_some(message(Action::Duplicate)),
                                false,
                            ),
                        ])
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                    ),
                    kit::row(
                        fl!("settings-widgets-remove-bar"),
                        Some(if editor.bars.len() > 1 {
                            fl!("settings-widgets-remove-bar-hint", bar = bar.name.as_str())
                        } else {
                            fl!("settings-widgets-last-bar")
                        }),
                        self.treated_action(
                            "settings-button-remove-bar",
                            fl!("settings-widgets-remove-short"),
                            (editor.bars.len() > 1).then_some(message(Action::RemoveBar)),
                            kit::Treatment::Destructive,
                        ),
                    ),
                ]),
            ));
        }
        // How the preview is drawn, last: it changes nothing that is saved.
        let preview_outputs = std::iter::once((String::new(), fl!("settings-widgets-any-monitor")))
            .chain(
                outputs
                    .iter()
                    .map(|output| (output.clone(), self.monitor_name(output))),
            )
            .collect();
        content = content.push(noted_group(
            fl!("settings-widgets-preview"),
            kit::rows([kit::row(
                fl!("settings-widgets-preview-monitor"),
                None,
                direction::row([
                    self.action(
                        "settings-button-refresh-monitors",
                        fl!("settings-widgets-refresh"),
                        (!self.refreshing).then_some(Message::BackendChanged),
                        false,
                    ),
                    pick(
                        "widgets-output",
                        preview_outputs,
                        Some(view.output.clone()),
                        |output| message(Action::Output(output)),
                    ),
                ])
                .spacing(8)
                .align_y(iced::Alignment::Center),
            )]),
            view.lock.then(|| fl!("settings-widgets-lock-hint")),
        ));
        content.into()
    }

    /// Under the preview: what the selected widget is and where, with the
    /// actions that also work without dragging, then adding a widget.
    fn selected_widget<'a>(&'a self, editor: &'a Editor) -> Element<'a, Message> {
        let view = &self.widget_layouts;
        let bar = &editor.bars[view.bar];
        let edge = (!view.lock).then_some(bar.edge.as_str());
        // Where the selected widget is, and its neighbours there.
        let place: Option<(String, Vec<String>)> = view.selected().and_then(|name| {
            if view.lock {
                lock_region(editor, name).map(|region| {
                    (
                        region.anchor.key().to_owned(),
                        region
                            .widgets
                            .iter()
                            .filter(|name| editor.lock_active(name))
                            .cloned()
                            .collect(),
                    )
                })
            } else {
                bar_zone(bar, name).map(|zone| (ZONES[zone].to_owned(), bar.zones[zone].clone()))
            }
        });
        let mut rows: Vec<Element<'a, Message>> = Vec::new();
        match (view.selected(), place) {
            (Some(name), Some((zone, neighbours))) => {
                let label = preview_label(name, view.lock);
                let position = neighbours.iter().position(|other| other == name);
                // Earlier and later read as the bar runs: up and down on a
                // side bar, left and right along a top or bottom one.
                let across = if view.lock {
                    lock_region(editor, name).is_some_and(|region| region.row)
                } else {
                    matches!(bar.edge.as_str(), "top" | "bottom")
                };
                let (earlier, later) = if across {
                    (
                        fl!("settings-widgets-move-left"),
                        fl!("settings-widgets-move-right"),
                    )
                } else {
                    (
                        fl!("settings-widgets-move-up"),
                        fl!("settings-widgets-move-down"),
                    )
                };
                let act = |action: Action| message(Action::For(name.to_owned(), Box::new(action)));
                // Only the moves that apply: none is shown greyed out.
                let mut line: Vec<Element<'a, Message>> = vec![
                    column![
                        text(label.clone())
                            .size(kit::size::BODY)
                            .font(kit::strong_font(iced::font::Weight::Semibold)),
                        text(if view.lock {
                            place_label(&zone)
                        } else {
                            fl!(
                                "settings-widgets-zone-heading",
                                zone = zone_label(&zone, edge)
                            )
                        })
                        .size(kit::size::CAPTION)
                        .style(canopa_ui::muted),
                    ]
                    .spacing(2)
                    .width(Length::Fill)
                    .into(),
                ];
                if position.is_some_and(|position| position > 0) {
                    line.push(self.action(
                        format!("widgets-{name}-earlier"),
                        earlier,
                        Some(act(Action::Shift(-1))),
                        false,
                    ));
                }
                if position.is_some_and(|position| position + 1 < neighbours.len()) {
                    line.push(self.action(
                        format!("widgets-{name}-later"),
                        later,
                        Some(act(Action::Shift(1))),
                        false,
                    ));
                }
                if name != "authentication" {
                    line.push(self.treated_action(
                        format!("widgets-{name}-remove"),
                        fl!("settings-widgets-remove-short"),
                        Some(act(Action::Remove)),
                        kit::Treatment::Destructive,
                    ));
                }
                rows.push(
                    direction::row(line)
                        .spacing(8)
                        .align_y(iced::Alignment::Center)
                        .into(),
                );
                let places: Vec<(String, String)> = if view.lock {
                    Anchor::ALL
                        .into_iter()
                        .map(|anchor| (anchor.key().to_owned(), place_label(anchor.key())))
                        .collect()
                } else {
                    ZONES
                        .iter()
                        .map(|zone| ((*zone).to_owned(), zone_label(zone, edge)))
                        .collect()
                };
                rows.push(kit::row(
                    fl!("settings-widgets-position"),
                    None,
                    super::sized_pick(
                        "widgets-placement",
                        places,
                        Some(zone.clone()),
                        kit::PICK_WIDTH,
                        |zone| message(Action::Place(zone)),
                    ),
                ));
                // Lock screen extras apply to the selected widget's region.
                if view.lock
                    && let Some(region) = lock_region(editor, name)
                {
                    rows.push(toggle(
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
                        rows.push(toggle(
                            "widgets-hide",
                            fl!("settings-widgets-hide"),
                            hidden,
                            message(Action::Hide(!hidden)),
                        ));
                    }
                }
            }
            _ => rows.push(kit::caption(fl!("settings-widgets-none-selected"))),
        }
        let placed: Vec<String> = if view.lock {
            editor
                .lock_catalog()
                .into_iter()
                .filter(|name| editor.lock_active(name))
                .collect()
        } else {
            bar.zones.iter().flatten().cloned().collect()
        };
        let available: Vec<String> = if view.lock {
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
        rows.push(kit::row(
            fl!("settings-widgets-add"),
            None,
            if available.is_empty() {
                kit::caption(fl!("settings-widgets-all-placed"))
            } else {
                hinted_choice(
                    "widgets-add",
                    available.iter().map(|name| word(name)).collect(),
                    None,
                    fl!("settings-widgets-add-placeholder"),
                    {
                        let available = available.clone();
                        move |label| message(Action::Add(named(&available, &label)))
                    },
                )
            },
        ));
        column![
            iced::widget::rule::horizontal(1).style(kit::divider),
            kit::rows(rows)
        ]
        .spacing(12)
        .into()
    }

    fn widget_preview<'a>(&'a self, editor: &'a Editor) -> Element<'a, Message> {
        responsive(move |size| self.draw_widget_preview(editor, size.width))
            .height(Length::Shrink)
            .into()
    }

    fn preview_geometry(&self, editor: &Editor, available_width: f32) -> Geometry {
        let view = &self.widget_layouts;
        Geometry::new(
            editor,
            &Sight {
                lock: view.lock,
                bar: view.bar,
                output: &view.output,
            },
            available_width,
        )
    }

    fn draw_widget_preview(&self, editor: &Editor, available_width: f32) -> Element<'_, Message> {
        let view = &self.widget_layouts;
        let geometry = self.preview_geometry(editor, available_width);
        let (width, height) = (geometry.width, geometry.height);
        if geometry.hidden {
            return container(kit::caption(fl!("settings-widgets-bar-hidden")))
                .width(Length::Fill)
                .padding(24)
                .center_x(Length::Fill)
                .style(zone_style(false))
                .into();
        }
        let dragging = view.drag.as_ref().filter(|drag| drag.pointer.is_some());
        let target = dragging.and_then(|drag| geometry.drop_at(drag.pointer?, &drag.name));
        let mut board = stack![space().width(width).height(height)];
        // Each zone or region: a quiet panel with its name.
        for cell in &geometry.cells {
            let lit = target.as_ref().is_some_and(|drop| drop.place == cell.key);
            board = board.push(positioned(
                cell.bounds.x,
                cell.bounds.y,
                container(space())
                    .width(cell.bounds.width)
                    .height(cell.bounds.height)
                    .style(zone_style(lit))
                    .into(),
            ));
            // A tiny screen showing where this zone or region sits.
            board = board.push(positioned(
                cell.bounds.x + PAD + 2.,
                cell.bounds.y + 9.,
                where_icon(&cell.key, geometry.edge.as_deref()),
            ));
            board = board.push(positioned(
                cell.bounds.x + PAD + 2. + ICON_W + 8.,
                cell.bounds.y + 9.,
                text(cell.label.clone())
                    .size(kit::size::SMALL)
                    .font(kit::strong_font(iced::font::Weight::Semibold))
                    .style(canopa_ui::muted)
                    .into(),
            ));
            if cell.widgets.is_empty() {
                board = board.push(positioned(
                    cell.bounds.x + PAD,
                    cell.bounds.y + HEADER,
                    container(
                        text(fl!("settings-widgets-drop-here"))
                            .size(kit::size::SMALL)
                            .style(canopa_ui::muted),
                    )
                    .width(cell.bounds.width - 2. * PAD)
                    .height(CHIP)
                    .center_x(cell.bounds.width - 2. * PAD)
                    .center_y(CHIP)
                    .into(),
                ));
            }
        }
        for placement in &geometry.placements {
            let bounds = placement.bounds;
            let name = placement.name.clone();
            let active = view.selected() == Some(name.as_str());
            let lifted = dragging.is_some_and(|drag| drag.name == name);
            let label = preview_label(&name, view.lock);
            let chip = chip(
                label.clone(),
                bounds.width,
                active,
                if lifted { 0.35 } else { 1.0 },
            );
            let chip = iced::widget::mouse_area(chip)
                .on_press(message(Action::Grab(name.clone())))
                .interaction(if dragging.is_some() {
                    iced::mouse::Interaction::Grabbing
                } else {
                    iced::mouse::Interaction::Grab
                });
            let select = message(Action::Select(name.clone()));
            let chip = canopa_ui::accessibility::semantic(
                canopa_ui::accessibility::Node::button(label)
                    .key(format!("widget-preview-{name}"))
                    .toggled(active)
                    .on_click(Some(select)),
                chip,
            );
            board = board.push(positioned(bounds.x, bounds.y, chip));
        }
        // Where the dragged widget would land, then the widget itself.
        if let (Some(drag), Some(drop)) = (dragging, &target)
            && let Some(line) = geometry.marker(drop, &drag.name)
        {
            board = board.push(positioned(
                line.x,
                line.y,
                container(space())
                    .width(line.width)
                    .height(line.height)
                    .style(|theme: &iced::Theme| container::Style {
                        background: Some(canopa_ui::colors(theme).accent.into()),
                        border: iced::border::rounded(2),
                        ..Default::default()
                    })
                    .into(),
            ));
        }
        if let Some(drag) = dragging
            && let Some(pointer) = drag.pointer
            && let Some(placement) = geometry
                .placements
                .iter()
                .find(|placement| placement.name == drag.name)
        {
            let bounds = placement.bounds;
            board = board.push(positioned(
                (pointer.x - bounds.width / 2.).clamp(0., (width - bounds.width).max(0.)),
                (pointer.y - CHIP / 2.).clamp(0., (height - CHIP).max(0.)),
                chip(
                    preview_label(&drag.name, view.lock),
                    bounds.width,
                    true,
                    0.95,
                ),
            ));
        }
        let surface = iced::widget::mouse_area(board.width(width).height(height))
            .on_move(|point| message(Action::Pointer(point)))
            .on_release(message(Action::Release(available_width)))
            .on_exit(message(Action::Leave));
        if dragging.is_some() {
            surface
                .interaction(iced::mouse::Interaction::Grabbing)
                .into()
        } else {
            surface.into()
        }
    }
}

/// The board's measures: a zone's name, a widget row, the gaps between.
const HEADER: f32 = 32.;
const CHIP: f32 = 32.;
const GAP: f32 = 6.;
const PAD: f32 = 8.;
const CELL_GAP: f32 = 10.;

/// What the board shows, independent of the rest of the page.
struct Sight<'a> {
    lock: bool,
    bar: usize,
    output: &'a str,
}

/// Where a dragged widget would go: a zone or lock screen region key, and
/// the widget it would go before (none: the end).
#[derive(Clone, Debug, PartialEq)]
struct Drop {
    place: String,
    before: Option<String>,
}

/// One column of the bar board, or one region of the lock screen grid.
struct Cell {
    key: String,
    label: String,
    bounds: Rect,
    widgets: Vec<String>,
}

/// The board laid out at a given width: the bar's three zones side by side
/// in the order they run, or the lock screen's nine regions as a grid, with
/// every placed widget as a row in its zone.
struct Geometry {
    width: f32,
    height: f32,
    /// The bar's screen edge; none on the lock screen.
    edge: Option<String>,
    /// The bar is hidden on the chosen monitor.
    hidden: bool,
    cells: Vec<Cell>,
    placements: Vec<Placement>,
}

impl Geometry {
    fn new(editor: &Editor, sight: &Sight<'_>, available_width: f32) -> Self {
        let width = available_width.max(240.);
        let bar = &editor.bars[sight.bar];
        let mut geometry = Self {
            width,
            height: 0.,
            edge: (!sight.lock).then(|| bar.edge.clone()),
            hidden: false,
            cells: Vec::new(),
            placements: Vec::new(),
        };
        // (key, label, widgets) for each cell, by rows of three.
        let cells: Vec<(String, String, Vec<String>)> = if sight.lock {
            let hidden = editor.lock.hidden_on(sight.output);
            Anchor::ALL
                .into_iter()
                .map(|anchor| {
                    let widgets = editor
                        .lock
                        .layout
                        .regions
                        .iter()
                        .filter(|region| region.anchor == anchor)
                        .flat_map(|region| &region.widgets)
                        .filter(|name| editor.lock_active(name) && !hidden.contains(name))
                        .cloned()
                        .collect();
                    (anchor.key().to_owned(), place_label(anchor.key()), widgets)
                })
                .collect()
        } else {
            if !sight.output.is_empty() && !bar.visible_on(sight.output) {
                geometry.hidden = true;
                return geometry;
            }
            ZONES
                .iter()
                .enumerate()
                .map(|(index, zone)| {
                    (
                        (*zone).to_owned(),
                        zone_label(zone, Some(&bar.edge)),
                        bar.zones[index].clone(),
                    )
                })
                .collect()
        };
        let column = (width - 2. * CELL_GAP) / 3.;
        let content =
            |widgets: &Vec<String>| HEADER + widgets.len().max(1) as f32 * (CHIP + GAP) + PAD - GAP;
        let mut top = 0.;
        for row in cells.chunks(3) {
            let height = row
                .iter()
                .map(|(_, _, widgets)| content(widgets))
                .fold(if sight.lock { 84. } else { 140. }, f32::max);
            for (index, (key, label, widgets)) in row.iter().enumerate() {
                let bounds = Rect {
                    x: index as f32 * (column + CELL_GAP),
                    y: top,
                    width: column,
                    height,
                };
                for (position, name) in widgets.iter().enumerate() {
                    geometry.placements.push(Placement {
                        name: name.clone(),
                        bounds: Rect {
                            x: bounds.x + PAD,
                            y: bounds.y + HEADER + position as f32 * (CHIP + GAP),
                            width: column - 2. * PAD,
                            height: CHIP,
                        },
                        scale: 1.,
                    });
                }
                geometry.cells.push(Cell {
                    key: key.clone(),
                    label: label.clone(),
                    bounds,
                    widgets: widgets.clone(),
                });
            }
            top += height + CELL_GAP;
        }
        geometry.height = (top - CELL_GAP).max(0.);
        geometry
    }

    /// The cell under `point`, or the nearest one.
    fn cell_at(&self, point: iced::Point) -> Option<&Cell> {
        let distance = |cell: &&Cell| {
            let b = cell.bounds;
            let dx = (b.x - point.x).max(point.x - (b.x + b.width)).max(0.);
            let dy = (b.y - point.y).max(point.y - (b.y + b.height)).max(0.);
            dx * dx + dy * dy
        };
        self.cells
            .iter()
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
    }

    /// The rows of `cell` other than `dragged`.
    fn chips<'a>(
        &'a self,
        cell: &'a Cell,
        dragged: &'a str,
    ) -> impl Iterator<Item = &'a Placement> {
        cell.widgets
            .iter()
            .filter(move |name| *name != dragged)
            .filter_map(|name| {
                self.placements
                    .iter()
                    .find(|placement| &placement.name == name)
            })
    }

    /// Where `dragged` lands when dropped at `point`: the zone under the
    /// pointer, before the first widget whose middle lies below it.
    fn drop_at(&self, point: iced::Point, dragged: &str) -> Option<Drop> {
        let cell = self.cell_at(point)?;
        let before = self
            .chips(cell, dragged)
            .find(|chip| chip.bounds.y + chip.bounds.height / 2. > point.y)
            .map(|chip| chip.name.clone());
        Some(Drop {
            place: cell.key.clone(),
            before,
        })
    }

    /// The line where a dropped widget would go.
    fn marker(&self, drop: &Drop, dragged: &str) -> Option<Rect> {
        let cell = self.cells.iter().find(|cell| cell.key == drop.place)?;
        let chips: Vec<&Placement> = self.chips(cell, dragged).collect();
        let y = match drop
            .before
            .as_ref()
            .and_then(|before| chips.iter().find(|chip| &chip.name == before))
        {
            Some(chip) => chip.bounds.y - GAP / 2. - 1.,
            None => chips
                .last()
                .map_or(cell.bounds.y + HEADER - GAP / 2. - 1., |last| {
                    last.bounds.y + CHIP + GAP / 2. - 1.
                }),
        };
        Some(Rect {
            x: cell.bounds.x + PAD,
            y,
            width: cell.bounds.width - 2. * PAD,
            height: 2.,
        })
    }
}

const ICON_W: f32 = 24.;
const ICON_H: f32 = 15.;

/// A screen in miniature: the bar along `edge` with zone `key` lit, or on
/// the lock screen (no edge) a dot where region `key` is.
fn where_icon<'a>(key: &str, edge: Option<&str>) -> Element<'a, Message> {
    let lit = |theme: &iced::Theme| canopa_ui::colors(theme).accent;
    let faint = |theme: &iced::Theme| {
        kit::blend(
            canopa_ui::colors(theme).surface,
            canopa_ui::colors(theme).text,
            0.3,
        )
    };
    let block = |w: f32, h: f32, on: bool| -> Element<'a, Message> {
        container(space())
            .width(w)
            .height(h)
            .style(move |theme: &iced::Theme| container::Style {
                background: Some(if on { lit(theme) } else { faint(theme) }.into()),
                border: iced::border::rounded(1),
                ..Default::default()
            })
            .into()
    };
    let mut parts = stack![space().width(ICON_W).height(ICON_H)];
    match edge {
        Some(edge) => {
            let zone = ZONES.iter().position(|zone| *zone == key).unwrap_or(0) as f32;
            let across = matches!(edge, "top" | "bottom");
            let (length, thick) = if across {
                (ICON_W - 4., 3.)
            } else {
                (ICON_H - 4., 3.)
            };
            let part = length / 3.;
            for index in 0..3 {
                let along = 2. + index as f32 * part;
                let (x, y, w, h) = match edge {
                    "top" => (along, 2., part - 1., thick),
                    "bottom" => (along, ICON_H - 2. - thick, part - 1., thick),
                    "right" => (ICON_W - 2. - thick, along, thick, part - 1.),
                    _ => (2., along, thick, part - 1.),
                };
                parts = parts.push(positioned(x, y, block(w, h, index as f32 == zone)));
            }
        }
        None => {
            let cell = Anchor::ALL
                .iter()
                .position(|anchor| anchor.key() == key)
                .unwrap_or(4);
            let (column, row) = ((cell % 3) as f32, (cell / 3) as f32);
            parts = parts.push(positioned(
                3. + column * (ICON_W - 10.) / 2.,
                2.5 + row * (ICON_H - 9.) / 2.,
                block(4., 4., true),
            ));
        }
    }
    container(parts)
        .style(|theme: &iced::Theme| container::Style {
            border: iced::Border {
                color: kit::blend(
                    canopa_ui::colors(theme).surface,
                    canopa_ui::colors(theme).text,
                    0.35,
                ),
                width: 1.0,
                radius: 3.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// A zone or region panel, lit while a widget is dragged over it.
fn zone_style(lit: bool) -> impl Fn(&iced::Theme) -> container::Style {
    move |theme| {
        let p = canopa_ui::colors(theme);
        container::Style {
            background: Some(
                if lit {
                    kit::blend(p.background, p.accent, 0.12)
                } else {
                    kit::blend(p.surface, p.background, 0.45)
                }
                .into(),
            ),
            border: iced::Border {
                color: if lit {
                    kit::blend(p.surface, p.accent, 0.6)
                } else {
                    kit::hairline(theme)
                },
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        }
    }
}

/// A widget on the board: a row with its name or a sample, and a grip.
fn chip<'a>(label: String, width: f32, active: bool, opacity: f32) -> Element<'a, Message> {
    container(
        direction::row([
            text(label)
                .size(kit::size::CAPTION)
                .wrapping(iced::widget::text::Wrapping::None)
                .width(Length::Fill)
                .into(),
            text("⠿")
                .size(kit::size::CAPTION)
                .style(canopa_ui::muted)
                .into(),
        ])
        .align_y(iced::Alignment::Center),
    )
    .padding([0, 10])
    .width(width)
    .height(CHIP)
    .center_y(CHIP)
    .style(move |theme: &iced::Theme| {
        let p = canopa_ui::colors(theme);
        let (background, border, text) = if active {
            (kit::selection(theme), p.accent, kit::on_selection(theme))
        } else {
            (p.raised, kit::blend(p.raised, p.border, 0.6), p.text)
        };
        container::Style {
            background: Some(background.scale_alpha(opacity).into()),
            text_color: Some(text.scale_alpha(opacity)),
            border: iced::Border {
                color: border.scale_alpha(opacity),
                width: if active { 1.5 } else { 1.0 },
                radius: kit::CONTROL_RADIUS.into(),
            },
            ..Default::default()
        }
    })
    .into()
}

/// A module name read as a word: "launcher" → "Launcher".
fn word(name: &str) -> String {
    let mut chars = name.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}
/// The module whose [`word`] is `label`.
fn named(names: &[String], label: &str) -> String {
    names
        .iter()
        .find(|name| word(name) == label)
        .cloned()
        .unwrap_or_else(|| label.to_owned())
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
        word(name)
    };
    sample.chars().take(19).collect()
}

fn positioned<'a>(x: f32, y: f32, element: Element<'a, Message>) -> Element<'a, Message> {
    column![space().height(y), row![space().width(x), element]].into()
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A state on the Widgets page with the shipped bar loaded.
    fn widgets() -> (tempfile::TempDir, State) {
        let (root, mut state) = super::super::tests::fresh();
        let editor = Editor::load(Snapshot::load(state.paths.common()).unwrap()).unwrap();
        state.update_widget_layouts(Action::Loaded(Ok(Box::new(editor))));
        (root, state)
    }

    fn editor(state: &State) -> &Editor {
        state
            .widget_layouts
            .editor
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
    }

    fn overlaps(placements: &[Placement]) -> bool {
        placements.iter().enumerate().any(|(index, a)| {
            placements[index + 1..].iter().any(|b| {
                let (a, b) = (a.bounds, b.bounds);
                a.x < b.x + b.width
                    && b.x < a.x + a.width
                    && a.y < b.y + b.height
                    && b.y < a.y + a.height
            })
        })
    }

    #[test]
    fn every_bar_edge_fits_its_widgets_without_overlap() {
        let (_root, mut state) = widgets();
        for edge in EDGES {
            state.update_widget_layouts(Action::Edge(edge.into()));
            let geometry = state.preview_geometry(editor(&state), 648.);
            assert_eq!(
                geometry.placements.len(),
                editor(&state).bars[0].zones.iter().flatten().count(),
                "{edge}"
            );
            assert!(
                !overlaps(&geometry.placements),
                "{edge} {:?}",
                geometry
                    .placements
                    .iter()
                    .map(|p| (p.name.clone(), p.bounds.x, p.bounds.width))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn dragging_a_widget_moves_it_before_the_one_it_is_dropped_on() {
        let (_root, mut state) = widgets();
        let bar = editor(&state).bars[0].clone();
        let (first, last) = (
            bar.zones[0][0].clone(),
            bar.zones[2].last().unwrap().clone(),
        );
        let geometry = state.preview_geometry(editor(&state), 648.);
        let at = |name: &str| {
            let b = geometry
                .placements
                .iter()
                .find(|placement| placement.name == name)
                .unwrap()
                .bounds;
            iced::Point::new(b.x + b.width / 2., b.y + b.height / 2. - 2.)
        };
        // The last widget of the end zone, dropped on the first of the
        // start zone, goes before it.
        state.update_widget_layouts(Action::Grab(last.clone()));
        state.update_widget_layouts(Action::Pointer(at(&last)));
        state.update_widget_layouts(Action::Pointer(at(&first)));
        state.update_widget_layouts(Action::Release(648.));
        let bar = &editor(&state).bars[0];
        assert_eq!(bar.zones[0][..2], [last.clone(), first]);
        assert!(!bar.zones[2].contains(&last));
        assert!(state.widget_layouts.drag.is_none());
        assert!(state.widget_layouts.dirty());
    }

    #[test]
    fn a_press_without_a_move_only_selects() {
        let (_root, mut state) = widgets();
        let before = editor(&state).bars[0].zones.clone();
        let name = before[2][0].clone();
        state.update_widget_layouts(Action::Grab(name.clone()));
        state.update_widget_layouts(Action::Release(648.));
        assert_eq!(editor(&state).bars[0].zones, before);
        assert_eq!(state.widget_layouts.widget.as_deref(), Some(name.as_str()));
        assert!(!state.widget_layouts.dirty());
    }

    #[test]
    fn a_widget_dropped_on_an_empty_lock_screen_region_opens_it() {
        let (_root, mut state) = widgets();
        state.update_widget_layouts(Action::Lock(true));
        let geometry = state.preview_geometry(editor(&state), 648.);
        let name = geometry
            .placements
            .iter()
            .find(|placement| placement.name != "authentication")
            .unwrap()
            .name
            .clone();
        // The top-right corner holds nothing in the shipped layout.
        assert!(
            editor(&state)
                .lock
                .layout
                .regions
                .iter()
                .all(|region| region.anchor != Anchor::TopRight || region.widgets.is_empty())
        );
        state.update_widget_layouts(Action::Grab(name.clone()));
        state.update_widget_layouts(Action::Pointer(iced::Point::new(geometry.width - 10., 10.)));
        state.update_widget_layouts(Action::Release(648.));
        let region = lock_region(editor(&state), &name).unwrap();
        assert_eq!(region.anchor, Anchor::TopRight);
    }
}
