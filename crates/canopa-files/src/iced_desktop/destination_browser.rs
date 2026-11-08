//! Read-only folder navigation inside a destination draft.
use super::*;
use iced::{
    Length,
    widget::{column, container, row, scrollable, text},
};

const PAGE_SIZE: usize = 40;

#[derive(Default)]
pub(super) struct Browser {
    scan: Option<Scan>,
    revision: u64,
}

struct Scan {
    path: PathBuf,
    result: Option<Arc<Result<Vec<PathBuf>, String>>>,
    cancellation: Cancellation,
    page: usize,
}

impl Drop for Scan {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    Browse,
    Navigate(u64, PathBuf),
    Ready(u64, Arc<Result<Vec<PathBuf>, String>>),
    Page(u64, usize),
    Use(u64),
    Cancel,
}

fn control(id: window::Id, name: &str) -> iced::widget::Id {
    iced::widget::Id::from(format!("files-destination-browser-{id:?}-{name}"))
}

impl Browser {
    pub fn open(&self) -> bool {
        self.scan.is_some()
    }

    pub fn update(
        &mut self,
        id: window::Id,
        event: Event,
        folder: &mut String,
        home: &Path,
        send: impl Fn(Event) -> Message + Send + 'static,
    ) -> Task<Message> {
        let path = match event {
            Event::Browse if self.scan.is_none() => {
                local_path(Path::new(folder), home, Some(home)).unwrap_or_else(|_| home.to_owned())
            }
            Event::Navigate(revision, path) if revision == self.revision && self.open() => path,
            Event::Ready(revision, result) if revision == self.revision => {
                if let Some(scan) = &mut self.scan {
                    scan.result = Some(result);
                }
                return Task::none();
            }
            Event::Page(revision, page) if revision == self.revision => {
                if let Some(scan) = &mut self.scan
                    && let Some(Ok(folders)) = scan.result.as_deref()
                    && page.saturating_mul(PAGE_SIZE) < folders.len()
                {
                    scan.page = page;
                    return iced::widget::operation::scroll_to(
                        control(id, "list"),
                        iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: 0.0 },
                    );
                }
                return Task::none();
            }
            Event::Use(revision) if revision == self.revision => {
                if let Some(scan) = self.scan.as_ref()
                    && matches!(scan.result.as_deref(), Some(Ok(_)))
                {
                    *folder = display_location(&scan.path);
                    return self.back(id);
                }
                return Task::none();
            }
            Event::Cancel if self.open() => return self.back(id),
            _ => return Task::none(),
        };

        self.revision += 1;
        let revision = self.revision;
        let cancellation = Cancellation::default();
        self.scan = Some(Scan {
            path: path.clone(),
            result: None,
            cancellation: cancellation.clone(),
            page: 0,
        });
        Task::perform(
            async move {
                Directory::read_cancellable(path, &cancellation)
                    .map(|directory| {
                        directory
                            .entries
                            .into_iter()
                            .filter(|entry| entry.is_directory() && !entry.is_hidden())
                            .map(|entry| entry.path)
                            .collect()
                    })
                    .map_err(|error| error.to_string())
            },
            move |result| send(Event::Ready(revision, Arc::new(result))),
        )
    }

    fn back(&mut self, id: window::Id) -> Task<Message> {
        self.scan = None;
        iced::widget::operation::focus(control(id, "browse"))
    }

    pub fn view<'a>(
        &'a self,
        desktop: &'a Desktop,
        id: window::Id,
        send: impl Fn(Event) -> Message,
    ) -> Option<Element<'a, Message>> {
        let scan = self.scan.as_ref()?;
        let appearance = &desktop.configuration.appearance;
        let font = canopa_ui::font(appearance);
        let small = appearance.typography().label_size.px();
        let revision = self.revision;
        let mut order = Vec::new();
        let mut reveal = Vec::new();
        let mut action =
            |name: &str, label: Element<'a, Message>, event: Option<Event>, treatment, in_list| {
                let widget = control(id, name);
                if event.is_some() {
                    order.push(widget.clone());
                    if in_list {
                        reveal.push((widget.clone(), control(id, "list")));
                    }
                }
                let message = event.map(&send);
                let button =
                    canopa_ui::styled_action(label, message.clone(), treatment, false, appearance);
                let button = if in_list {
                    button.padding([5, 10])
                } else {
                    button
                };
                focus::control(widget, message, appearance, button)
            };
        let label = |value: &'static str| text(value).size(small).font(font).into();
        let ordinary = canopa_ui::Action::Normal;
        let tools = row![
            action(
                "home",
                label("Home"),
                Some(Event::Navigate(revision, desktop.home.clone())),
                ordinary,
                false
            ),
            action(
                "up",
                label("Up"),
                scan.path
                    .parent()
                    .map(|path| Event::Navigate(revision, path.to_owned())),
                ordinary,
                false
            ),
        ]
        .spacing(6);
        let mut folders = column![].spacing(2);
        let mut pages = None;
        match scan.result.as_deref() {
            Some(Ok(paths)) if !paths.is_empty() => {
                let start = scan.page * PAGE_SIZE;
                let end = (start + PAGE_SIZE).min(paths.len());
                for (index, path) in paths[start..end].iter().enumerate() {
                    let name = path.file_name().unwrap_or(path.as_os_str());
                    let name = name
                        .to_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("{name:?}"));
                    let entry = row![
                        desktop.icons.chrome(chrome::Icon::Folder, 17.0, 0.9),
                        chrome::single_line(name, font, small + 1.0, None, true),
                        desktop.icons.chrome(chrome::Icon::Forward, 14.0, 0.8),
                    ]
                    .spacing(8)
                    .align_y(iced::Alignment::Center);
                    folders = folders.push(action(
                        &format!("item-{index}"),
                        entry.into(),
                        Some(Event::Navigate(revision, path.clone())),
                        canopa_ui::Action::Quiet,
                        true,
                    ));
                }
                if paths.len() > PAGE_SIZE {
                    pages = Some(
                        row![]
                            .push(action(
                                "previous",
                                label("Previous"),
                                scan.page
                                    .checked_sub(1)
                                    .map(|page| Event::Page(revision, page)),
                                ordinary,
                                false,
                            ))
                            .push(action(
                                "next",
                                label("Next"),
                                (end < paths.len()).then_some(Event::Page(revision, scan.page + 1)),
                                ordinary,
                                false,
                            ))
                            .spacing(6),
                    );
                }
            }
            Some(Ok(_)) => folders = folders.push(label("No subfolders")),
            Some(Err(error)) => {
                folders = folders.push(
                    text(error)
                        .font(font)
                        .size(small)
                        .wrapping(text::Wrapping::WordOrGlyph),
                )
            }
            None => folders = folders.push(label("Reading folders…")),
        }
        let buttons = row![
            action(
                "back",
                label("Back"),
                Some(Event::Cancel),
                canopa_ui::Action::Quiet,
                false
            ),
            action(
                "use",
                label("Use this folder"),
                matches!(scan.result.as_deref(), Some(Ok(_))).then_some(Event::Use(revision)),
                canopa_ui::Action::Primary,
                false
            ),
        ]
        .spacing(8);
        let body = container(
            column![
                chrome::heading_with_action(
                    desktop,
                    chrome::Icon::Folder,
                    "Choose a folder",
                    "",
                    Some(tools.into())
                ),
                chrome::single_line(
                    super::question_dialog::display_location(&desktop.home, &scan.path),
                    font,
                    small + 1.0,
                    Some(chrome::ink(appearance, canopa_theme::ColorRole::Text)),
                    true,
                ),
                scrollable(folders)
                    .id(control(id, "list"))
                    .direction(scrollable::Direction::Vertical(
                        scrollable::Scrollbar::new()
                            .width(6)
                            .scroller_width(6)
                            .spacing(6),
                    ))
                    .height(Length::Fill),
                pages,
                super::question_dialog::decision_actions(buttons, None),
            ]
            .spacing(8),
        )
        .padding(16)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| {
            canopa_ui::frame(appearance, appearance.style(canopa_theme::UiKind::Panel))
        });
        let default = matches!(scan.result.as_deref(), Some(Ok(_))).then(|| control(id, "use"));
        Some(focus::group(body, order, reveal, default))
    }
}

pub(super) fn browse_id(id: window::Id) -> iced::widget::Id {
    control(id, "browse")
}

pub(super) fn browse_button<'a>(
    desktop: &'a Desktop,
    id: window::Id,
    message: Option<Message>,
) -> Element<'a, Message> {
    let appearance = &desktop.configuration.appearance;
    focus::control(
        browse_id(id),
        message.clone(),
        appearance,
        canopa_ui::styled_action(
            text("Browse…")
                .font(canopa_ui::font(appearance))
                .size(appearance.typography().label_size.px()),
            message,
            canopa_ui::Action::Normal,
            false,
            appearance,
        ),
    )
}
