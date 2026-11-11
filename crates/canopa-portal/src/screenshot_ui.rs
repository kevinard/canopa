//! Selection of an immutable capture; the helper never publishes an image file.
pub use crate::screenshot_image::{DialogInput, Selection};
use crate::{
    screenshot::Kind,
    screenshot_image::{Canvas, Rect},
};
use iced::{
    Element, Event, Length, Size, Subscription, Task, Theme, keyboard,
    widget::{Space, column, container, row, text},
    window,
};
use std::{
    fs::File,
    sync::{Arc, Mutex},
};
mod scene;

pub fn run(
    input: DialogInput,
    appearance: canopa_theme::Appearance,
) -> anyhow::Result<Option<Selection>> {
    anyhow::ensure!(
        input.app_id.len() <= 4096 && input.parent_window.len() <= 4096,
        "Invalid screenshot dialog"
    );
    let file = Arc::new(input.source.open()?);
    let images: Vec<_> = input
        .source
        .previews(&file)?
        .into_iter()
        .map(|image| {
            iced::widget::image::Handle::from_rgba(image.width(), image.height(), image.into_raw())
        })
        .collect();
    let canvas = Canvas::new(&input.source.frames)?;
    let theme = canopa_ui::theme(appearance.clone());
    let font = canopa_ui::font(&appearance);
    let result = Arc::new(Mutex::new(None));
    let output = result.clone();
    iced::daemon(
        move || {
            let settings = window::Settings {
                size: Size::new(900.0, 680.0),
                min_size: Some(Size::new(520.0, 420.0)),
                exit_on_close_request: false,
                platform_specific: window::settings::PlatformSpecific {
                    application_id: "org.canopa.Portal".into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let parent = input
                .parent_window
                .strip_prefix("wayland:")
                .unwrap_or_default()
                .to_owned();
            let (_, open) = window::open_dialog(parent, input.modal, settings);
            let mut dialog = Dialog {
                input: input.clone(),
                file: file.clone(),
                images: images.clone(),
                canvas: canvas.clone(),
                theme: theme.clone(),
                output: output.clone(),
                mode: input.kind,
                selected: None,
                cursor: None,
                drag: None,
                completed: false,
            };
            dialog.reset(input.kind);
            (dialog, open.map(|_| Message::Opened))
        },
        Dialog::update,
        Dialog::view,
    )
    .title(|app: &Dialog, _| {
        let title = if app.mode == Kind::Color {
            fl!("portal-screenshot-color-window")
        } else {
            fl!("portal-screenshot-window")
        };
        fl!("portal-window-title", title = title)
    })
    .theme(|app: &Dialog, _| app.theme.clone())
    .subscription(Dialog::subscription)
    .default_font(font)
    .run()
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let selected = result.lock().expect("screenshot result").clone();
    Ok(selected)
}

struct Dialog {
    input: DialogInput,
    file: Arc<File>,
    images: Vec<iced::widget::image::Handle>,
    canvas: Canvas,
    theme: Theme,
    output: Arc<Mutex<Option<Selection>>>,
    mode: Kind,
    selected: Option<Selection>,
    cursor: Option<(u32, u32)>,
    drag: Option<(u32, u32)>,
    completed: bool,
}
#[derive(Clone, Debug)]
enum Message {
    Opened,
    Mode(Kind),
    Hover(Option<(u32, u32)>),
    Press((u32, u32)),
    Release(Option<(u32, u32)>),
    NextDisplay,
    ChooseWindow,
    Apply,
    Cancel,
    Event(Event),
}
impl Dialog {
    fn reset(&mut self, kind: Kind) {
        self.mode = kind;
        self.drag = None;
        let first = &self.input.source.frames[0];
        let b = self.canvas.bounds(first);
        self.cursor = Some((
            (b.x + b.width / 2.0).floor() as u32,
            (b.y + b.height / 2.0).floor() as u32,
        ));
        self.selected = match kind {
            Kind::Screen | Kind::Window | Kind::ActiveWindow => Some(Selection::Image(None)),
            Kind::Area => None,
            Kind::Color => Some(Selection::Pixel {
                frame: 0,
                x: first.width / 2,
                y: first.height / 2,
            }),
        };
    }
    fn subscription(&self) -> Subscription<Message> {
        iced::event::listen_with(|event, status, _| {
            let escape = matches!(
                &event,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                })
            );
            (escape || status == iced::event::Status::Ignored).then_some(Message::Event(event))
        })
    }
    fn valid(&self) -> bool {
        match self.selected {
            Some(Selection::Pixel { frame, x, y }) => {
                self.input.source.pixel(&self.file, frame, x, y).is_ok()
            }
            Some(Selection::Image(rect)) => {
                let rect = rect.unwrap_or(self.canvas.full());
                rect.validate(&self.canvas).is_ok()
                    && self.canvas.has_pixels(&self.input.source.frames, rect)
            }
            None | Some(Selection::ChooseWindow) => false,
        }
    }
    fn area(&mut self, point: (u32, u32)) {
        if let Some(start) = self.drag {
            let x = start.0.min(point.0);
            let y = start.1.min(point.1);
            self.selected = Some(Selection::Image(Some(Rect {
                x,
                y,
                width: start.0.max(point.0) - x + 1,
                height: start.1.max(point.1) - y + 1,
            })));
        }
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        if self.completed {
            return Task::none();
        }
        match message {
            Message::Apply if self.valid() => {
                *self.output.lock().expect("screenshot decision") = self.selected.clone();
                self.completed = true;
                return iced::exit();
            }
            Message::Cancel | Message::Event(Event::Window(window::Event::CloseRequested)) => {
                self.completed = true;
                return iced::exit();
            }
            Message::ChooseWindow if self.input.allow_area && self.input.allow_window => {
                *self.output.lock().expect("screenshot decision") = Some(Selection::ChooseWindow);
                self.completed = true;
                return iced::exit();
            }
            Message::Mode(kind)
                if self.input.allow_area && matches!(kind, Kind::Screen | Kind::Area) =>
            {
                self.reset(kind)
            }
            Message::Hover(Some(point)) => {
                self.cursor = Some(point);
                self.area(point);
            }
            Message::Press(point) => {
                self.cursor = Some(point);
                if self.mode == Kind::Area {
                    self.drag = Some(point);
                    self.area(point);
                }
                if self.mode == Kind::Color
                    && let Some((frame, x, y)) = self.canvas.pixel_at(
                        &self.input.source.frames,
                        point.0 as f64 + 0.5,
                        point.1 as f64 + 0.5,
                    )
                {
                    self.selected = Some(Selection::Pixel { frame, x, y });
                }
            }
            Message::Release(point) => {
                if let Some(point) = point.or(self.cursor) {
                    self.area(point);
                }
                self.drag = None;
            }
            Message::NextDisplay => {
                if let Some(Selection::Pixel { frame, .. }) = self.selected {
                    let next = (frame + 1) % self.input.source.frames.len();
                    let f = &self.input.source.frames[next];
                    self.selected = Some(Selection::Pixel {
                        frame: next,
                        x: f.width / 2,
                        y: f.height / 2,
                    });
                }
            }
            Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
                key, modifiers, ..
            })) => {
                use keyboard::key::Named;
                if modifiers.alt()
                    && matches!(&key, keyboard::Key::Character(c) if c.eq_ignore_ascii_case("w"))
                {
                    return self.update(Message::ChooseWindow);
                }
                if key == keyboard::Key::Named(Named::Escape) {
                    return self.update(Message::Cancel);
                }
                if key == keyboard::Key::Named(Named::Tab) {
                    return if modifiers.shift() {
                        iced::widget::operation::focus_previous()
                    } else {
                        iced::widget::operation::focus_next()
                    };
                }
                if key == keyboard::Key::Named(Named::Space) && self.mode == Kind::Area {
                    if self.drag.is_some() {
                        self.drag = None;
                    } else if let Some(point) = self.cursor {
                        self.drag = Some(point);
                        self.area(point);
                    }
                }
                let step = if modifiers.shift() { 10 } else { 1 };
                let delta = match key {
                    keyboard::Key::Named(Named::ArrowLeft) => (-step, 0),
                    keyboard::Key::Named(Named::ArrowRight) => (step, 0),
                    keyboard::Key::Named(Named::ArrowUp) => (0, -step),
                    keyboard::Key::Named(Named::ArrowDown) => (0, step),
                    _ => (0, 0),
                };
                if delta != (0, 0) {
                    if let Some(Selection::Pixel { frame, x, y }) = self.selected {
                        let f = &self.input.source.frames[frame];
                        self.selected = Some(Selection::Pixel {
                            frame,
                            x: x.saturating_add_signed(delta.0).min(f.width - 1),
                            y: y.saturating_add_signed(delta.1).min(f.height - 1),
                        });
                    } else if let Some(point) = self.cursor {
                        let next = (
                            point
                                .0
                                .saturating_add_signed(delta.0)
                                .min(self.canvas.width - 1),
                            point
                                .1
                                .saturating_add_signed(delta.1)
                                .min(self.canvas.height - 1),
                        );
                        self.cursor = Some(next);
                        self.area(next);
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }
    fn view(&self, _: window::Id) -> Element<'_, Message> {
        let requester = if self.input.app_id.is_empty() {
            fl!("portal-requesting-app")
        } else {
            let short: String = self.input.app_id.chars().take(120).collect();
            if short.len() < self.input.app_id.len() {
                format!("{short}…")
            } else {
                short
            }
        };
        let title = if self.mode == Kind::Color {
            fl!("portal-screenshot-color-title")
        } else {
            fl!("portal-screenshot-title")
        };
        let instruction = match self.mode {
            Kind::Screen => fl!("portal-screenshot-screen-hint"),
            Kind::Window | Kind::ActiveWindow => fl!("portal-screenshot-window-hint"),
            Kind::Area => fl!("portal-screenshot-area-hint"),
            Kind::Color => fl!("portal-screenshot-color-hint"),
        };
        let mut top = column![
            text(title).size(22),
            text(fl!("portal-requested-by", requester = requester)).style(canopa_ui::muted)
        ]
        .spacing(8);
        if self.input.allow_area {
            let mut modes = row![
                canopa_ui::action(
                    text(fl!("portal-screenshot-mode-screen")),
                    Some(Message::Mode(Kind::Screen)),
                    canopa_ui::Action::Normal,
                    self.mode == Kind::Screen
                ),
                canopa_ui::action(
                    text(fl!("portal-screenshot-mode-area")),
                    Some(Message::Mode(Kind::Area)),
                    canopa_ui::Action::Normal,
                    self.mode == Kind::Area
                ),
            ]
            .spacing(8);
            if self.input.allow_window {
                modes = modes.push(canopa_ui::action(
                    text(fl!("portal-screenshot-mode-window")),
                    Some(Message::ChooseWindow),
                    canopa_ui::Action::Normal,
                    false,
                ));
            }
            top = top.push(modes);
        }
        if matches!(self.mode, Kind::Window | Kind::ActiveWindow) {
            top = top.push(text(&self.input.source.frames[0].name).size(14));
        }
        top = top.push(text(instruction).size(13).style(canopa_ui::muted));
        let status: Element<'_, Message> = match self.selected {
            Some(Selection::Pixel { frame, x, y }) => {
                let rgba = self
                    .input
                    .source
                    .pixel(&self.file, frame, x, y)
                    .unwrap_or([0; 4]);
                let color = iced::Color::from_rgb8(rgba[0], rgba[1], rgba[2]);
                let swatch = container(Space::new().width(28).height(28))
                    .style(move |_| container::Style::default().background(color));
                let mut line: Vec<Element<'_, Message>> = vec![
                    swatch.into(),
                    text(fl!(
                        "portal-screenshot-pixel",
                        color = format!("#{:02X}{:02X}{:02X}", rgba[0], rgba[1], rgba[2]),
                        x = x.to_string(),
                        y = y.to_string(),
                        display = self.input.source.frames[frame].name.as_str()
                    ))
                    .size(14)
                    .into(),
                ];
                if self.input.source.frames.len() > 1 {
                    line.push(
                        canopa_ui::action(
                            text(fl!("portal-screenshot-next-display")),
                            Some(Message::NextDisplay),
                            canopa_ui::Action::Normal,
                            false,
                        )
                        .into(),
                    );
                }
                canopa_ui::direction::row(line).spacing(10).into()
            }
            Some(Selection::Image(rect)) => {
                let rect = rect.unwrap_or(self.canvas.full());
                text(if self.valid() {
                    fl!(
                        "portal-pixel-size",
                        width = rect.width.to_string(),
                        height = rect.height.to_string()
                    )
                } else {
                    fl!("portal-screenshot-too-large")
                })
                .style(canopa_ui::muted)
                .into()
            }
            None | Some(Selection::ChooseWindow) => text(fl!("portal-screenshot-select-area"))
                .style(canopa_ui::muted)
                .into(),
        };
        let scene = scene::Scene {
            frames: &self.input.source.frames,
            images: &self.images,
            canvas: &self.canvas,
            selected: self.selected.as_ref(),
            cursor: self.cursor,
            dragging: self.drag.is_some(),
            mode: self.mode,
            accent: self.theme.extended_palette().primary.base.color,
        };
        let actions = canopa_ui::direction::row([
            canopa_ui::action(
                text(fl!("portal-cancel")),
                Some(Message::Cancel),
                canopa_ui::Action::Normal,
                false,
            )
            .width(Length::Fill)
            .into(),
            canopa_ui::action(
                text(if self.mode == Kind::Color {
                    fl!("portal-screenshot-use-color")
                } else {
                    fl!("portal-share")
                }),
                self.valid().then_some(Message::Apply),
                canopa_ui::Action::Normal,
                false,
            )
            .width(Length::Fill)
            .into(),
        ])
        .spacing(12);
        container(column![top, Element::new(scene), status, actions].spacing(14))
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(canopa_ui::surface)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, os::fd::AsRawFd};
    fn dialog(kind: Kind) -> Dialog {
        let pixels = vec![
            10, 20, 30, 255, 40, 50, 60, 255, 70, 80, 90, 255, 100, 110, 120, 255,
        ];
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(&pixels).unwrap();
        let frames = vec![crate::screenshot_image::Frame {
            name: "display".into(),
            logical: crate::capture::LogicalRect {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
            },
            width: 2,
            height: 2,
            offset: 0,
        }];
        let canvas = Canvas::new(&frames).unwrap();
        let input = DialogInput {
            app_id: "test".into(),
            parent_window: String::new(),
            modal: true,
            kind,
            allow_area: kind == Kind::Screen,
            allow_window: false,
            source: crate::screenshot_image::Source {
                descriptor: file.as_raw_fd(),
                bytes: 16,
                frames,
            },
        };
        let mut result = Dialog {
            input,
            file: Arc::new(file),
            images: vec![iced::widget::image::Handle::from_rgba(2, 2, pixels)],
            canvas,
            theme: canopa_ui::theme(canopa_theme::Appearance::default()),
            output: Arc::new(Mutex::new(None)),
            mode: kind,
            selected: None,
            cursor: None,
            drag: None,
            completed: false,
        };
        result.reset(kind);
        result
    }
    #[test]
    fn choosing_a_window_is_a_target_change_and_never_screen_acceptance() {
        let mut screen = dialog(Kind::Screen);
        let _ = screen.update(Message::ChooseWindow);
        assert!(!screen.completed);
        screen.input.allow_window = true;
        let _ = screen.update(Message::ChooseWindow);
        assert!(screen.completed);
        assert_eq!(
            *screen.output.lock().unwrap(),
            Some(Selection::ChooseWindow)
        );
        for kind in [Kind::Window, Kind::ActiveWindow] {
            let mut window = dialog(kind);
            window.input.allow_window = true;
            let _ = window.update(Message::ChooseWindow);
            let _ = window.update(Message::Mode(Kind::Area));
            assert!(!window.completed);
            assert_eq!(window.mode, kind);
            let _ = window.update(Message::Apply);
            assert_eq!(*window.output.lock().unwrap(), Some(Selection::Image(None)));
        }
    }
    #[test]
    fn color_requires_confirmation_and_completed_decisions_cannot_change() {
        let mut app = dialog(Kind::Color);
        let _ = app.update(Message::Opened);
        assert!(app.output.lock().unwrap().is_none());
        let _ = app.update(Message::Press((0, 0)));
        assert!(app.output.lock().unwrap().is_none());
        let _ = app.update(Message::Apply);
        assert_eq!(
            *app.output.lock().unwrap(),
            Some(Selection::Pixel {
                frame: 0,
                x: 0,
                y: 0
            })
        );
        let _ = app.update(Message::Press((1, 1)));
        let _ = app.update(Message::Cancel);
        assert_eq!(
            *app.output.lock().unwrap(),
            Some(Selection::Pixel {
                frame: 0,
                x: 0,
                y: 0
            })
        );
    }
    #[test]
    fn area_needs_a_nonempty_selection_and_cancel_never_confirms_one() {
        let mut app = dialog(Kind::Area);
        let _ = app.update(Message::Apply);
        assert!(!app.completed);
        let _ = app.update(Message::Press((0, 0)));
        let _ = app.update(Message::Hover(Some((1, 1))));
        let _ = app.update(Message::Release(Some((1, 1))));
        assert_eq!(
            app.selected,
            Some(Selection::Image(Some(Rect {
                x: 0,
                y: 0,
                width: 2,
                height: 2
            })))
        );
        let _ = app.update(Message::Cancel);
        let _ = app.update(Message::Apply);
        assert!(app.output.lock().unwrap().is_none());
    }
    #[test]
    fn explicit_screen_target_cannot_be_changed_to_area_by_a_helper_message() {
        let mut app = dialog(Kind::Screen);
        app.input.allow_area = false;
        let _ = app.update(Message::Mode(Kind::Area));
        assert_eq!(app.mode, Kind::Screen);
        let _ = app.update(Message::Apply);
        assert_eq!(*app.output.lock().unwrap(), Some(Selection::Image(None)));
    }
    fn key(named: keyboard::key::Named, modifiers: keyboard::Modifiers) -> Message {
        Message::Event(Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(named),
            modified_key: keyboard::Key::Named(named),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Enter),
            location: keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        }))
    }
    #[test]
    fn ignored_enter_never_confirms_and_keyboard_moves_exact_source_pixels() {
        use keyboard::key::Named;
        let mut app = dialog(Kind::Color);
        let _ = app.update(key(Named::Enter, keyboard::Modifiers::empty()));
        assert!(app.output.lock().unwrap().is_none());
        let _ = app.update(key(Named::ArrowLeft, keyboard::Modifiers::empty()));
        assert_eq!(
            app.selected,
            Some(Selection::Pixel {
                frame: 0,
                x: 0,
                y: 1
            })
        );
        let mut area = dialog(Kind::Area);
        let _ = area.update(key(Named::Space, keyboard::Modifiers::empty()));
        let _ = area.update(key(Named::ArrowLeft, keyboard::Modifiers::empty()));
        let _ = area.update(key(Named::ArrowUp, keyboard::Modifiers::empty()));
        let _ = area.update(key(Named::Space, keyboard::Modifiers::empty()));
        assert_eq!(
            area.selected,
            Some(Selection::Image(Some(Rect {
                x: 0,
                y: 0,
                width: 2,
                height: 2
            })))
        );
        assert!(area.output.lock().unwrap().is_none());
    }
}
