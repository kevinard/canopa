//! Shared presentation for the four session authentication surfaces.
//!
//! Three arrangements share one visual language:
//! - the full-screen form (sign-in and unlock): a centred identity, a pill
//!   field with its submit button, status text and system controls at the
//!   bottom corners of the screen;
//! - the full-screen sheet (choosers, power and password-change steps): a card
//!   with the dialog heading;
//! - the dialog (authorization and passphrase windows): heading, request card,
//!   field and a footer whose actions are anchored to the bottom edge.
use super::iced::Scene;
use super::raster::Emblem;
use super::{Control, ControlKind, SessionView, wrap};
use crate::layout::Rect;
use canopa_theme::ColorRole;
use iced::font::Weight;

pub const DIALOG_SIZE: (u32, u32) = (560, 416);
pub const AUTHORIZATION_DIALOG_SIZE: (u32, u32) = (560, 392);
pub const DIALOG_MARGIN: f32 = 16.;
/// Dialog windows fit their content between these logical heights.
pub const DIALOG_MIN_HEIGHT: u32 = 280;
pub const DIALOG_MAX_HEIGHT: u32 = 560;

#[derive(Debug)]
pub struct AuthenticationPanel {
    /// Reserve room in centered clock-and-form stacks without drawing extra card space.
    pub height_reserve: f32,
    pub heading: String,
    /// One short line below the heading.
    pub subtitle: String,
    pub emblem: Emblem,
    pub dialog: bool,
    pub choices: &'static [(usize, Emblem)],
    pub utilities: &'static [usize],
    pub disclosure: Option<usize>,
    pub list_actions: bool,
    pub checked_control: Option<usize>,
    pub choice_details: std::collections::BTreeMap<usize, String>,
    /// The account that authenticates, shown in a dialog's request card.
    pub account: Option<String>,
    /// Show the chosen identity with its primary action in place of an echoed
    /// field; the consumer reveals the field again when typing starts.
    pub confirm_identity: bool,
}

/// "Account: {name}", as the request card and identity choices show it.
pub fn account_label(name: &str) -> String {
    fl!("session-account", name = name)
}

/// How a line of copy is coloured.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Muted,
    /// Request text in a dialog body.
    Body,
    /// A status message: the danger colour after a failure.
    Notice,
    /// The Caps Lock warning.
    Caps,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Form,
    Sheet,
    Dialog,
}

/// Full-screen form width and the avatar's diameter.
const FORM_WIDTH: f32 = 360.;
const AVATAR: f32 = 88.;
const FIELD: f32 = 52.;
const SHEET_WIDTH: f32 = 404.;
const SHEET_PADDING: f32 = 24.;
const DIALOG_PADDING: f32 = 16.;
const BUTTON: f32 = 42.;
const CHIP: f32 = 40.;
const ACCOUNT_ROW: f32 = 76.;

enum Content<'a> {
    /// Form: avatar, name and subtitle, centred.
    Identity,
    /// Sheet and dialog: emblem tile, title and subtitle.
    Heading,
    Copy(Vec<String>, Tone),
    Request(Request<'a>),
    Divider,
    Field {
        label: Vec<String>,
        placeholder: String,
        submit: Option<&'a Control>,
    },
    /// Form: a full-width button in the field's place while no response is asked.
    Pill(&'a Control),
    /// Form: centred text actions below the field.
    Links(Vec<&'a Control>),
    /// Form: accounts to sign in as, the selected one highlighted.
    Accounts(Vec<&'a Control>),
    Buttons(Vec<&'a Control>, Treatment),
    /// System controls in the column when the screen corners hold other widgets.
    System,
}

struct Request<'a> {
    subject: Option<&'a str>,
    lines: Vec<String>,
    account: Option<&'a str>,
    policy: bool,
    /// A policy request names who asked, or the policy's source.
    context: bool,
}

/// One caption-and-value section of a request card.
const ACCOUNT_SECTION: f32 = 62.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Treatment {
    Choice,
    Action,
    Utility,
}

enum SystemItem<'a> {
    Desktop(&'a Control),
    /// A static label without a control when the layout cannot be switched.
    Keyboard(Option<&'a Control>),
    Power(&'a Control),
    /// A passive status, such as the battery level, beside the controls.
    /// The level (0 to 1) fills a battery glyph.
    Status(Emblem, String, Option<f32>),
}

pub(super) struct Plan<'a> {
    panel: &'a AuthenticationPanel,
    mode: Mode,
    width: f32,
    height: f32,
    height_reserve: f32,
    username: bool,
    avatar: bool,
    rows: Vec<(f32, Content<'a>)>,
    footer: Vec<&'a Control>,
    footer_disclosure: Option<&'a Control>,
    system: Vec<SystemItem<'a>>,
    /// System controls are in the column instead of at the screen corners.
    system_inline: bool,
}

impl AuthenticationPanel {
    fn control_height(&self, treatment: Treatment) -> f32 {
        match treatment {
            Treatment::Utility => 36.,
            Treatment::Choice if self.list_actions => 56.,
            Treatment::Choice => 44.,
            Treatment::Action if self.dialog => BUTTON,
            Treatment::Action => 44.,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn plan<'a>(
        &'a self,
        view: &'a SessionView,
        keyboard: Option<&'a Control>,
        keyboard_inline: bool,
        username: bool,
        avatar: bool,
        pin_origin: bool,
        corners_free: bool,
        dialog_height: f32,
    ) -> Plan<'a> {
        let mode = if self.dialog {
            Mode::Dialog
        } else if self.list_actions || view.hide_input {
            Mode::Sheet
        } else {
            Mode::Form
        };
        let mut plan = Plan {
            panel: self,
            mode,
            width: match mode {
                Mode::Form => FORM_WIDTH,
                Mode::Sheet => SHEET_WIDTH,
                Mode::Dialog => DIALOG_SIZE.0 as f32 - 2. * DIALOG_MARGIN,
            },
            height: match mode {
                Mode::Form => 0.,
                Mode::Sheet => SHEET_PADDING,
                Mode::Dialog => DIALOG_PADDING,
            },
            height_reserve: if self.dialog {
                dialog_height - 2. * DIALOG_MARGIN
            } else if pin_origin && self.height_reserve > 0. {
                self.height_reserve
            } else {
                0.
            },
            username,
            avatar,
            rows: Vec::new(),
            footer: Vec::new(),
            footer_disclosure: None,
            system: Vec::new(),
            system_inline: false,
        };
        let mut choices = Vec::new();
        let mut actions = Vec::new();
        let mut utilities = Vec::new();
        let mut links = Vec::new();
        let mut accounts = Vec::new();
        let mut disclosure = None;
        for control in &view.controls {
            match &control.kind {
                ControlKind::DesktopChooser { .. } if !self.dialog => {
                    plan.system.push(SystemItem::Desktop(control));
                    continue;
                }
                ControlKind::Power if !self.dialog => {
                    plan.system.push(SystemItem::Power(control));
                    continue;
                }
                ControlKind::AccountChooser { .. } if mode == Mode::Form => {
                    links.push(control);
                    continue;
                }
                ControlKind::Account { .. } if mode == Mode::Form => {
                    accounts.push(control);
                    continue;
                }
                _ => {}
            }
            if self.disclosure == Some(control.id) {
                disclosure = Some(control);
            } else if self.choices.iter().any(|(id, _)| *id == control.id) {
                choices.push(control);
            } else if self.utilities.contains(&control.id) {
                utilities.push(control);
            } else {
                actions.push(control);
            }
        }
        if !self.dialog && !view.layout.is_empty() && (keyboard_inline || keyboard.is_some()) {
            // Keep the keyboard beside power, after the desktop chooser.
            let at = plan
                .system
                .iter()
                .position(|item| matches!(item, SystemItem::Power(_)))
                .unwrap_or(plan.system.len());
            plan.system.insert(at, SystemItem::Keyboard(keyboard));
        } else if let Some(keyboard) = keyboard.filter(|_| self.dialog) {
            utilities.push(keyboard);
        }
        match mode {
            Mode::Form => plan.form(view, actions, choices, utilities, links, accounts),
            Mode::Sheet => plan.sheet(view, actions, choices, utilities),
            Mode::Dialog => plan.dialog(view, actions, choices, utilities, disclosure),
        }
        if !self.dialog && !plan.system.is_empty() && !corners_free {
            plan.system_inline = true;
            plan.push(Content::System, 20., CHIP);
        }
        plan.height += match mode {
            Mode::Form => 0.,
            Mode::Sheet => SHEET_PADDING,
            Mode::Dialog => DIALOG_PADDING,
        };
        plan
    }
}

/// A short prompt becomes the field's placeholder; a longer one is shown above it.
fn field_text(view: &SessionView, columns: usize) -> (Vec<String>, String) {
    let Some(prompt) = &view.prompt else {
        return (Vec::new(), String::new());
    };
    let text = prompt.text.trim().trim_end_matches(':').trim();
    if !text.is_empty() && !text.contains('\n') && text.chars().count() <= 30 {
        (Vec::new(), text.to_owned())
    } else {
        (
            wrap(text, columns, 6),
            if prompt.echo {
                fl!("session-placeholder-type-response")
            } else {
                fl!("session-placeholder-enter-response")
            },
        )
    }
}

impl<'a> Plan<'a> {
    fn push(&mut self, row: Content<'a>, gap: f32, height: f32) {
        self.height += gap;
        self.rows.push((self.height, row));
        self.height += height;
    }

    fn buttons(&mut self, buttons: &[&'a Control], treatment: Treatment, gap: f32, height: f32) {
        for row in buttons.chunks(2) {
            self.push(Content::Buttons(row.to_vec(), treatment), gap, height);
        }
    }

    fn copy(&mut self, lines: Vec<String>, gap: f32, tone: Tone) {
        if !lines.is_empty() {
            let line = if self.mode == Mode::Form { 21. } else { 22. };
            let height = line * lines.len() as f32;
            self.push(Content::Copy(lines, tone), gap, height);
        }
    }

    fn status(&mut self, view: &SessionView, columns: usize, gap: f32) {
        if !view.message.is_empty() {
            self.copy(wrap(&view.message, columns, 6), gap, Tone::Notice);
        }
        if view.caps {
            let gap = if view.message.is_empty() { gap } else { 2. };
            self.copy(vec![fl!("session-caps-lock-on")], gap, Tone::Caps);
        }
    }

    fn form(
        &mut self,
        view: &'a SessionView,
        mut actions: Vec<&'a Control>,
        choices: Vec<&'a Control>,
        utilities: Vec<&'a Control>,
        links: Vec<&'a Control>,
        accounts: Vec<&'a Control>,
    ) {
        if !accounts.is_empty() {
            let rows = accounts.len() as f32;
            self.push(
                Content::Accounts(accounts),
                30.,
                rows * ACCOUNT_ROW + (rows - 1.) * 10.,
            );
            self.status(view, 46, 14.);
            if !links.is_empty() {
                self.push(Content::Links(links), 14., 36.);
            }
            self.buttons(&utilities, Treatment::Utility, 8., 36.);
            return;
        }
        let (_, subtitle) = self.titles(view);
        // Space the identity apart from a clock and date stacked above it.
        self.push(
            Content::Identity,
            30.,
            AVATAR + 16. + 30. + if subtitle.is_empty() { 0. } else { 28. },
        );
        let primary = actions
            .iter()
            .position(|control| view.primary_control == Some(control.id))
            .map(|index| actions.remove(index));
        if view.prompt.is_some() && !self.panel.confirm_identity {
            let (label, placeholder) = field_text(view, 44);
            let label_height = if label.is_empty() {
                0.
            } else {
                21. * label.len() as f32 + 10.
            };
            self.push(
                Content::Field {
                    label,
                    placeholder,
                    submit: primary,
                },
                26.,
                label_height + FIELD,
            );
        } else if let Some(control) =
            primary.or_else(|| (!actions.is_empty()).then(|| actions.remove(0)))
        {
            self.push(Content::Pill(control), 26., FIELD);
        }
        self.status(view, 46, 14.);
        if !links.is_empty() {
            self.push(Content::Links(links), 16., 36.);
        }
        for row in actions.chunks(2) {
            self.push(Content::Buttons(row.to_vec(), Treatment::Action), 12., 44.);
        }
        self.buttons(&choices, Treatment::Choice, 12., 44.);
        self.buttons(&utilities, Treatment::Utility, 8., 36.);
    }

    fn sheet(
        &mut self,
        view: &'a SessionView,
        actions: Vec<&'a Control>,
        choices: Vec<&'a Control>,
        utilities: Vec<&'a Control>,
    ) {
        let panel = self.panel;
        self.push(Content::Heading, 0., 46.);
        if !panel.list_actions
            && let Some(prompt) = &view.prompt
        {
            self.copy(wrap(&prompt.text, 46, 6), 18., Tone::Muted);
        }
        self.status(view, 46, 16.);
        for (index, row) in actions
            .chunks(if panel.list_actions { 1 } else { 2 })
            .enumerate()
        {
            let mut row = row.to_vec();
            if row.len() == 2 && view.primary_control == Some(row[0].id) {
                row.swap(0, 1);
            }
            let treatment = if panel.list_actions {
                Treatment::Choice
            } else {
                Treatment::Action
            };
            self.push(
                Content::Buttons(row, treatment),
                if index == 0 { 20. } else { 8. },
                panel.control_height(treatment),
            );
        }
        if !choices.is_empty() {
            self.push(Content::Divider, 18., 1.);
            self.buttons(&choices, Treatment::Choice, 12., 44.);
        }
        if panel.list_actions && !utilities.is_empty() {
            self.push(Content::Divider, 16., 1.);
        }
        self.buttons(
            &utilities,
            Treatment::Utility,
            if panel.list_actions { 12. } else { 10. },
            36.,
        );
    }

    fn dialog(
        &mut self,
        view: &'a SessionView,
        actions: Vec<&'a Control>,
        choices: Vec<&'a Control>,
        utilities: Vec<&'a Control>,
        disclosure: Option<&'a Control>,
    ) {
        let panel = self.panel;
        self.push(Content::Heading, 0., 46.);
        let lines: Vec<_> = ["message-0", "message-1"]
            .iter()
            .filter_map(|key| view.custom.get(*key).filter(|s| !s.is_empty()))
            .flat_map(|text| wrap(text, 58, 8))
            .flat_map(|line| {
                let groups: Vec<_> = line.split_ascii_whitespace().collect();
                if hex_groups(&groups) && groups.len() > 6 {
                    let half = groups.len().div_ceil(2);
                    vec![groups[..half].join(" "), groups[half..].join(" ")]
                } else {
                    vec![line]
                }
            })
            .collect();
        let subject = view.custom.get("subject").map(String::as_str);
        let account = panel.account.as_deref();
        let policy = matches!(panel.emblem, Emblem::Shield);
        if subject.is_some() || account.is_some() {
            let context = policy
                && ["subject-name", "policy-publisher", "subject-process"]
                    .iter()
                    .any(|key| view.custom.contains_key(*key));
            let request = Request {
                subject,
                lines,
                account,
                policy,
                context,
            };
            let height = request.height();
            self.push(Content::Request(request), 22., height);
        } else {
            self.copy(lines, 18., Tone::Body);
        }
        self.buttons(&utilities, Treatment::Utility, 10., 36.);
        self.buttons(&choices, Treatment::Choice, 12., 44.);
        if view.prompt.is_some() && !view.hide_input {
            // Dialog fields keep their label visible while typing.
            let (mut label, mut placeholder) = field_text(view, 62);
            if label.is_empty() {
                label.push(std::mem::take(&mut placeholder));
            }
            let label_height = if label.is_empty() {
                0.
            } else {
                21. * label.len() as f32 + 8.
            };
            self.push(
                Content::Field {
                    label,
                    placeholder,
                    submit: None,
                },
                22.,
                label_height + BUTTON,
            );
        }
        self.status(view, 64, 10.);
        let (primary, mut secondary): (Vec<_>, Vec<_>) = actions
            .into_iter()
            .partition(|control| view.primary_control == Some(control.id));
        secondary.extend(primary);
        self.footer = secondary;
        self.footer_disclosure = disclosure;
    }

    /// The identity title and subtitle shown by the form and the sheet heading.
    fn titles(&self, view: &'a SessionView) -> (&'a str, &'a str) {
        let panel = self.panel;
        if self.username {
            (view.username.as_str(), panel.subtitle.as_str())
        } else {
            (
                view.custom
                    .get("request-title")
                    .map_or(panel.heading.as_str(), String::as_str),
                if panel.subtitle.is_empty() {
                    view.custom.get("heading").map_or("", String::as_str)
                } else {
                    panel.subtitle.as_str()
                },
            )
        }
    }

    /// Show a passive status among the system controls when they sit at the
    /// screen corners. Returns false when the status stays a widget.
    pub(super) fn add_status(&mut self, icon: Emblem, text: String, level: Option<f32>) -> bool {
        if self.system_inline || self.mode == Mode::Dialog || self.system.is_empty() {
            return false;
        }
        let at = self
            .system
            .iter()
            .position(|item| matches!(item, SystemItem::Keyboard(_) | SystemItem::Power(_)))
            .unwrap_or(self.system.len());
        self.system
            .insert(at, SystemItem::Status(icon, text, level));
        true
    }

    pub(super) fn size(&self, viewport: f32) -> (f32, f32) {
        let height = self.content_height();
        (
            self.width,
            if self.mode == Mode::Form {
                height
            } else {
                height.min(viewport)
            },
        )
    }

    fn content_height(&self) -> f32 {
        let height = self.height.max(self.height_reserve);
        if self.mode == Mode::Dialog
            && (!self.footer.is_empty() || self.footer_disclosure.is_some())
        {
            let footer_top =
                (height - DIALOG_PADDING - BUTTON).max(self.height + 26. - DIALOG_PADDING);
            height.max(footer_top + BUTTON + DIALOG_PADDING)
        } else {
            height
        }
    }

    /// The dialog window height that shows this content without spare space,
    /// keeping one status line free so a failure message never resizes it.
    pub(super) fn natural_dialog_height(&self, view: &SessionView) -> f32 {
        let _ = view;
        let status = 0.;
        let footer = if self.footer.is_empty() && self.footer_disclosure.is_none() {
            0.
        } else {
            26. + BUTTON
        };
        self.height + status + footer + 2. * DIALOG_MARGIN
    }

    pub(super) fn populate(
        &self,
        view: &SessionView,
        settings: &crate::settings::Settings,
        scene: &mut Scene,
        bounds: Rect,
        scale: f32,
        screen: iced::Size,
    ) {
        self.populate_content(view, settings, scene, bounds, scale, screen, true);
        // Scrolling content never owns viewport corner controls.
    }
    #[allow(clippy::too_many_arguments)]
    fn populate_content(
        &self,
        view: &SessionView,
        settings: &crate::settings::Settings,
        scene: &mut Scene,
        bounds: Rect,
        scale: f32,
        screen: iced::Size,
        corners: bool,
    ) {
        let content_height = self.content_height() * scale;
        if self.mode != Mode::Form && content_height > bounds.height + 1. {
            let mut content = scene.empty();
            let size = iced::Size::new(bounds.width, content_height);
            self.populate_content(
                view,
                settings,
                &mut content,
                Rect {
                    x: 0.,
                    y: 0.,
                    width: bounds.width,
                    height: content_height,
                },
                scale,
                size,
                false,
            );
            scene.scroll(content, size, bounds);
            if corners {
                self.populate_corners(view, settings, scene, scale, screen);
            }
            return;
        }
        let palette = settings.appearance.palette();
        let padding = match self.mode {
            Mode::Form => 0.,
            Mode::Sheet => SHEET_PADDING,
            Mode::Dialog => DIALOG_PADDING,
        };
        if self.mode == Mode::Sheet {
            scene.frame(
                Rect {
                    height: self.height * scale,
                    ..bounds
                },
                24. * scale,
                palette.surface,
                palette.border,
                scale,
            );
        }
        let left = bounds.x + padding * scale;
        let width = bounds.width - 2. * padding * scale;
        let (title, subtitle) = self.titles(view);
        let footer_top = (self.height_reserve.max(self.height) - DIALOG_PADDING - BUTTON)
            .max(self.height + 26. - DIALOG_PADDING);
        let spare = if self.mode == Mode::Dialog {
            ((footer_top - (self.height - DIALOG_PADDING) - 24.) / 3.).clamp(0., 14.)
        } else {
            0.
        };
        for (index, (offset, content)) in self.rows.iter().enumerate() {
            let top = bounds.y + (offset + if index > 0 { spare } else { 0. }) * scale;
            let row = Rect {
                x: left,
                y: top,
                width,
                height: 44. * scale,
            };
            match content {
                Content::Identity => {
                    let avatar = Rect {
                        x: left + (width - AVATAR * scale) / 2.,
                        y: top,
                        width: AVATAR * scale,
                        height: AVATAR * scale,
                    };
                    if (self.avatar || self.mode == Mode::Form)
                        && (view.user_avatar.is_some()
                            || view.avatar.is_some()
                            || view.avatar_initials.is_some())
                    {
                        scene.avatar(view, avatar, settings);
                    } else {
                        scene.frame(
                            avatar,
                            AVATAR * scale / 2.,
                            palette.surface,
                            palette.border,
                            scale,
                        );
                        scene.icon(
                            Emblem::Person,
                            Rect {
                                x: avatar.x + 24. * scale,
                                y: avatar.y + 24. * scale,
                                width: 40. * scale,
                                height: 40. * scale,
                            },
                            palette.muted,
                        );
                    }
                    scene.label(
                        title,
                        Rect {
                            y: top + (AVATAR + 16.) * scale,
                            height: 30. * scale,
                            ..row
                        },
                        24. * scale,
                        palette.text,
                        true,
                        Weight::Semibold,
                    );
                    scene.label(
                        subtitle,
                        Rect {
                            y: top + (AVATAR + 50.) * scale,
                            height: 24. * scale,
                            ..row
                        },
                        14. * scale,
                        palette.muted,
                        true,
                        Weight::Normal,
                    );
                }
                Content::Heading => {
                    scene.icon(
                        self.panel.emblem,
                        Rect {
                            x: if canopa_i18n::direction().is_rtl() {
                                left + width - 36. * scale
                            } else {
                                left
                            },
                            width: 36. * scale,
                            height: 36. * scale,
                            ..row
                        },
                        palette.accent,
                    );
                    scene.label(
                        title,
                        Rect {
                            x: left
                                + if canopa_i18n::direction().is_rtl() {
                                    0.
                                } else {
                                    50. * scale
                                },
                            width: width - 50. * scale,
                            height: 25. * scale,
                            ..row
                        },
                        21. * scale,
                        palette.text,
                        false,
                        Weight::Semibold,
                    );
                    scene.label(
                        subtitle,
                        Rect {
                            x: left
                                + if canopa_i18n::direction().is_rtl() {
                                    0.
                                } else {
                                    50. * scale
                                },
                            y: top + 27. * scale,
                            width: width - 50. * scale,
                            height: 20. * scale,
                        },
                        13. * scale,
                        palette.muted,
                        false,
                        Weight::Normal,
                    );
                }
                Content::Copy(lines, tone) => {
                    let color = match tone {
                        Tone::Notice if view.failed => palette.danger,
                        Tone::Caps => palette.warning,
                        Tone::Body => palette.text,
                        _ => palette.muted,
                    };
                    let line = if self.mode == Mode::Form { 21. } else { 22. } * scale;
                    scene.lines(
                        lines,
                        Rect {
                            height: line * lines.len() as f32,
                            ..row
                        },
                        14. * scale,
                        line,
                        color,
                        self.mode == Mode::Form,
                        Weight::Normal,
                    );
                }
                Content::Request(request) => {
                    let height = request.height() * scale;
                    scene.frame(
                        Rect { height, ..row },
                        12. * scale,
                        palette.raised,
                        palette.border,
                        scale,
                    );
                    if !request.policy
                        && let Some(groups) = fingerprint_groups(&request.lines)
                    {
                        let area = Rect {
                            x: left + 16. * scale,
                            width: width - 32. * scale,
                            height: 20. * scale,
                            ..row
                        };
                        let mut base = top;
                        if let Some(subject) = request.subject {
                            let (name, details) = key_subject(subject);
                            scene.label(
                                fl!("session-key-identity"),
                                Rect {
                                    y: top + 10. * scale,
                                    ..area
                                },
                                13. * scale,
                                palette.muted,
                                false,
                                Weight::Medium,
                            );
                            scene.label(
                                name,
                                Rect {
                                    y: top + 31. * scale,
                                    height: 24. * scale,
                                    ..area
                                },
                                15.5 * scale,
                                palette.text,
                                false,
                                Weight::Semibold,
                            );
                            if !details.is_empty() {
                                scene.label(
                                    details.join(" · "),
                                    Rect {
                                        y: top + 53. * scale,
                                        ..area
                                    },
                                    14. * scale,
                                    palette.muted,
                                    false,
                                    Weight::Normal,
                                );
                            }
                            base += KEY_IDENTITY * scale;
                            scene.frame(
                                Rect {
                                    y: base,
                                    height: scale,
                                    ..area
                                },
                                0.,
                                palette.border,
                                palette.border,
                                0.,
                            );
                        }
                        scene.label(
                            fl!("session-fingerprint"),
                            Rect {
                                y: base + 11. * scale,
                                ..area
                            },
                            13. * scale,
                            palette.muted,
                            false,
                            Weight::Medium,
                        );
                        for (index, group) in groups.iter().enumerate() {
                            scene.code(
                                group.trim(),
                                Rect {
                                    y: base + (34. + 22. * index as f32) * scale,
                                    height: 22. * scale,
                                    ..area
                                },
                                14.5 * scale,
                                palette.text,
                            );
                        }
                        continue;
                    }
                    let mut y = top + 12. * scale;
                    if let Some(subject) = request.subject {
                        scene.label(
                            subject,
                            Rect {
                                x: left + 14. * scale,
                                y,
                                width: width - 28. * scale,
                                height: 24. * scale,
                            },
                            15. * scale,
                            palette.text,
                            false,
                            Weight::Semibold,
                        );
                        y += 27. * scale;
                    }
                    if !request.lines.is_empty() {
                        scene.lines(
                            &request.lines,
                            Rect {
                                x: left + 14. * scale,
                                y,
                                width: width - 28. * scale,
                                height: 21. * request.lines.len() as f32 * scale,
                            },
                            14. * scale,
                            21. * scale,
                            palette.text,
                            false,
                            Weight::Normal,
                        );
                        y += 21. * request.lines.len() as f32 * scale;
                    }
                    if let Some(account) = request.account {
                        scene.avatar(
                            view,
                            Rect {
                                x: if canopa_i18n::direction().is_rtl() {
                                    left + width - 46. * scale
                                } else {
                                    left + 14. * scale
                                },
                                y: y + 4. * scale,
                                width: 32. * scale,
                                height: 32. * scale,
                            },
                            settings,
                        );
                        scene.label(
                            account_label(account),
                            Rect {
                                x: left
                                    + if canopa_i18n::direction().is_rtl() {
                                        14.
                                    } else {
                                        58.
                                    } * scale,
                                y,
                                width: width - 72. * scale,
                                height: 40. * scale,
                            },
                            14. * scale,
                            palette.text,
                            false,
                            Weight::Semibold,
                        );
                        y += 45. * scale;
                    }
                    if request.context {
                        let context = view
                            .custom
                            .get("subject-name")
                            .map(|s| fl!("session-requested-by") + ": " + s)
                            .or_else(|| {
                                view.custom
                                    .get("policy-publisher")
                                    .map(|s| fl!("session-policy-source") + ": " + s)
                            })
                            .or_else(|| {
                                view.custom
                                    .get("subject-process")
                                    .map(|s| fl!("session-requested-for") + ": " + s)
                            });
                        if let Some(context) = context {
                            scene.label(
                                context,
                                Rect {
                                    x: left + 14. * scale,
                                    y,
                                    width: width - 28. * scale,
                                    height: 40. * scale,
                                },
                                13. * scale,
                                palette.muted,
                                false,
                                Weight::Normal,
                            );
                        }
                    }
                }
                Content::Divider => scene.frame(
                    Rect {
                        height: scale,
                        ..row
                    },
                    0.,
                    palette.border,
                    palette.border,
                    0.,
                ),
                Content::Field {
                    label,
                    placeholder,
                    submit,
                } => {
                    let label_height = if label.is_empty() {
                        0.
                    } else {
                        21. * label.len() as f32 + 8.
                    };
                    if !label.is_empty() {
                        scene.lines(
                            label,
                            Rect {
                                height: label_height * scale,
                                ..row
                            },
                            14. * scale,
                            21. * scale,
                            palette.text,
                            false,
                            Weight::Normal,
                        );
                    }
                    let field = Rect {
                        y: top + label_height * scale,
                        height: if self.mode == Mode::Form {
                            FIELD
                        } else {
                            BUTTON
                        } * scale,
                        ..row
                    };
                    let radius = if self.mode == Mode::Form {
                        FIELD / 2.
                    } else {
                        10.
                    } * scale;
                    scene.frame(
                        field,
                        radius,
                        palette.surface,
                        if view.prompt.is_some() {
                            settings.appearance.role(ColorRole::Focus)
                        } else {
                            palette.border
                        },
                        scale,
                    );
                    let reveal_width = if view.can_reveal() { 40. * scale } else { 0. };
                    let submit_width = if submit.is_some() { 44. * scale } else { 0. };
                    scene.field(
                        view,
                        settings,
                        Rect {
                            x: field.x
                                + 3. * scale
                                + if canopa_i18n::direction().is_rtl() {
                                    reveal_width + submit_width
                                } else {
                                    0.
                                },
                            y: field.y + 3. * scale,
                            width: (field.width - reveal_width - submit_width - 6. * scale).max(1.),
                            height: field.height - 6. * scale,
                        },
                        scale,
                        placeholder,
                    );
                    if view.can_reveal() {
                        let control = Control::new(
                            crate::input::REVEAL_CONTROL,
                            if view.revealed() {
                                fl!("session-hide-response")
                            } else {
                                fl!("session-show-response")
                            },
                        );
                        let rect = Rect {
                            x: if canopa_i18n::direction().is_rtl() {
                                field.x + submit_width
                            } else {
                                field.x + field.width - submit_width - reveal_width
                            },
                            y: field.y + 4. * scale,
                            width: reveal_width,
                            height: field.height - 8. * scale,
                        };
                        scene.icon_button(
                            view,
                            settings,
                            &control,
                            if view.revealed() {
                                Emblem::EyeOff
                            } else {
                                Emblem::Eye
                            },
                            rect,
                            scale,
                        );
                    }
                    if let Some(control) = submit {
                        scene.icon_button(
                            view,
                            settings,
                            control,
                            Emblem::Arrow,
                            Rect {
                                x: if canopa_i18n::direction().is_rtl() {
                                    field.x + 4. * scale
                                } else {
                                    field.x + field.width - submit_width
                                },
                                y: field.y + 4. * scale,
                                width: submit_width - 4. * scale,
                                height: field.height - 8. * scale,
                            },
                            scale,
                        );
                    }
                }
                Content::Pill(control) => scene.button(
                    view,
                    settings,
                    control,
                    Rect {
                        height: FIELD * scale,
                        ..row
                    },
                    scale,
                    false,
                ),
                Content::Links(controls) => {
                    self.populate_buttons(view, settings, scene, controls, row, scale, true);
                }
                Content::Accounts(controls) => {
                    for (index, control) in controls.iter().enumerate() {
                        scene.button(
                            view,
                            settings,
                            control,
                            Rect {
                                y: top + index as f32 * (ACCOUNT_ROW + 10.) * scale,
                                height: ACCOUNT_ROW * scale,
                                ..row
                            },
                            scale,
                            false,
                        );
                    }
                }
                Content::Buttons(controls, treatment) => {
                    self.populate_buttons(
                        view,
                        settings,
                        scene,
                        controls,
                        Rect {
                            height: self.panel.control_height(*treatment) * scale,
                            ..row
                        },
                        scale,
                        *treatment == Treatment::Utility,
                    );
                }
                Content::System => self.populate_system(view, settings, scene, row, scale, true),
            }
        }
        if self.mode == Mode::Dialog
            && (!self.footer.is_empty() || self.footer_disclosure.is_some())
        {
            let mut controls = self.footer.clone();
            if let Some(disclosure) = self.footer_disclosure {
                controls.insert(0, disclosure);
            }
            self.populate_buttons(
                view,
                settings,
                scene,
                &controls,
                Rect {
                    x: left,
                    y: bounds.y + footer_top * scale,
                    width,
                    height: BUTTON * scale,
                },
                scale,
                false,
            );
        }
        if corners {
            self.populate_corners(view, settings, scene, scale, screen);
        }
    }
    fn populate_corners(
        &self,
        view: &SessionView,
        settings: &crate::settings::Settings,
        scene: &mut Scene,
        scale: f32,
        screen: iced::Size,
    ) {
        if !self.system_inline && !self.system.is_empty() {
            let margin = settings.layout.margin * scale;
            self.populate_system(
                view,
                settings,
                scene,
                Rect {
                    x: margin,
                    y: screen.height - margin - CHIP * scale,
                    width: screen.width - 2. * margin,
                    height: CHIP * scale,
                },
                scale,
                false,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn populate_buttons(
        &self,
        view: &SessionView,
        settings: &crate::settings::Settings,
        scene: &mut Scene,
        controls: &[&Control],
        row: Rect,
        scale: f32,
        quiet: bool,
    ) {
        let gap = 10. * scale;
        let width = (row.width - gap * controls.len().saturating_sub(1) as f32)
            / controls.len().max(1) as f32;
        for (index, control) in controls.iter().enumerate() {
            let column = if canopa_i18n::direction().is_rtl() {
                controls.len() - 1 - index
            } else {
                index
            };
            let mut control = (*control).clone();
            if self.panel.checked_control == Some(control.id) {
                control.label.push_str("  ✓");
            }
            scene.button_detail(
                view,
                settings,
                &control,
                Rect {
                    x: row.x + (width + gap) * column as f32,
                    width,
                    ..row
                },
                scale,
                quiet,
                self.panel
                    .choice_details
                    .get(&control.id)
                    .map(String::as_str),
            );
        }
    }

    fn populate_system(
        &self,
        view: &SessionView,
        settings: &crate::settings::Settings,
        scene: &mut Scene,
        row: Rect,
        scale: f32,
        centered: bool,
    ) {
        let palette = settings.appearance.palette();
        let mut chips = Vec::new();
        for item in &self.system {
            let (control, label, icon, start, level) = match item {
                SystemItem::Desktop(c) => (Some(*c), c.label.clone(), Emblem::Display, true, None),
                SystemItem::Keyboard(c) => (*c, view.layout.clone(), Emblem::Keyboard, false, None),
                SystemItem::Power(c) => (Some(*c), String::new(), Emblem::Power, false, None),
                SystemItem::Status(icon, label, level) => {
                    (None, label.clone(), *icon, false, *level)
                }
            };
            let width = if label.is_empty() {
                CHIP * scale
            } else {
                (view.text_width(&label, 14. * scale) + 48. * scale).min(240. * scale)
            };
            chips.push((control, label, icon, start, width, level));
        }
        let total = chips.iter().map(|c| c.4).sum::<f32>()
            + 10. * scale * chips.len().saturating_sub(1) as f32;
        let mut left = if centered {
            row.x + (row.width - total) / 2.
        } else {
            row.x
        };
        let mut right = row.x + row.width;
        for (control, label, icon, start, width, level) in chips {
            let from_start = if canopa_i18n::direction().is_rtl() {
                !start
            } else {
                start
            };
            let x = if centered || from_start {
                let x = left;
                left += width + 10. * scale;
                x
            } else {
                right -= width;
                let x = right;
                right -= 10. * scale;
                x
            };
            let rect = Rect { x, width, ..row };
            if let Some(control) = control {
                if label.is_empty() {
                    scene.icon_button(view, settings, control, icon, rect, scale);
                } else {
                    let mut visible = control.clone();
                    visible.label = label;
                    scene.button(view, settings, &visible, rect, scale, false);
                }
            } else {
                let rtl = canopa_i18n::direction().is_rtl();
                let side = 20. * scale;
                scene.status_icon(
                    icon,
                    Rect {
                        x: if rtl {
                            rect.x + rect.width - side
                        } else {
                            rect.x
                        },
                        y: rect.y + (rect.height - side) / 2.,
                        width: side,
                        height: side,
                    },
                    palette.muted,
                    level,
                );
                scene.label(
                    label,
                    Rect {
                        x: rect.x + if rtl { 0. } else { side + 6. * scale },
                        width: (rect.width - side - 6. * scale).max(1.),
                        ..rect
                    },
                    14. * scale,
                    palette.muted,
                    true,
                    Weight::Normal,
                );
            }
        }
    }
}

impl Request<'_> {
    fn height(&self) -> f32 {
        if !self.policy && fingerprint_groups(&self.lines).is_some() {
            return if self.subject.is_some() {
                KEY_IDENTITY
            } else {
                0.
            } + 90.;
        }
        let mut height = 0.;
        if self.subject.is_some() || !self.lines.is_empty() {
            height = 27. - 7.
                + if self.subject.is_some() { 23. } else { 0. }
                + 21. * self.lines.len() as f32
                + if self.account.is_some() { 5. } else { 14. };
            if self.subject.is_some() && self.lines.is_empty() && self.account.is_none() {
                height -= 2.;
            }
        }
        if self.account.is_some() {
            height += ACCOUNT_SECTION + if self.context { ACCOUNT_SECTION } else { 0. };
        }
        height
    }
}

fn key_subject(subject: &str) -> (&str, Vec<&str>) {
    let mut parts = subject.split(" · ");
    let name = parts.next().unwrap_or(subject);
    (name, parts.take(2).collect())
}

/// The height of a key card's identity part, above its fingerprint.
const KEY_IDENTITY: f32 = 84.;

fn hex_groups(groups: &[&str]) -> bool {
    groups.len() >= 4
        && groups
            .iter()
            .all(|group| group.len() == 4 && group.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn fingerprint_groups(lines: &[String]) -> Option<&[String]> {
    let (label, groups) = lines.split_first()?;
    (!groups.is_empty()
        && label
            .trim_end_matches(':')
            .eq_ignore_ascii_case("fingerprint")
        && groups
            .iter()
            .all(|group| hex_groups(&group.split_ascii_whitespace().collect::<Vec<_>>())))
    .then_some(groups)
}

#[cfg(test)]
mod tests {
    use super::super::iced::tests::rasterize;
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn fingerprint_card_keeps_identity_caption_and_monospace_hex_rows() {
        let _english = crate::i18n::english();
        let settings = Settings::load(
            &canopa_config::Request::Defaults,
            canopa_config::Component::Pinentry,
        )
        .unwrap()
        .0;
        let view = SessionView::new(&settings, "Alice".into()).unwrap();
        let panel = AuthenticationPanel {
            height_reserve: 0.,
            heading: "OpenPGP".into(),
            subtitle: String::new(),
            emblem: Emblem::Key,
            dialog: true,
            choices: &[],
            utilities: &[],
            disclosure: None,
            list_actions: false,
            checked_control: None,
            choice_details: Default::default(),
            account: None,
            confirm_identity: false,
        };
        let groups = ["00DD 88BB 0123 4567 89AB", "CDEF 0123 4567 89AB CDEF"];
        let request = Request {
            subject: Some("Alice · alice@example.com · RSA"),
            lines: std::iter::once("Fingerprint:".to_owned())
                .chain(groups.iter().map(|s| s.to_string()))
                .collect(),
            account: None,
            policy: false,
            context: false,
        };
        let plan = Plan {
            panel: &panel,
            mode: Mode::Dialog,
            width: 528.,
            height: request.height() + 32.,
            height_reserve: 0.,
            username: false,
            avatar: false,
            rows: vec![(16., Content::Request(request))],
            footer: vec![],
            footer_disclosure: None,
            system: vec![],
            system_inline: false,
        };
        let size = iced::Size::new(528., plan.height);
        let mut scene = Scene::new(view.assets.clone());
        plan.populate(
            &view,
            &settings,
            &mut scene,
            Rect {
                x: 0.,
                y: 0.,
                width: size.width,
                height: size.height,
            },
            1.,
            size,
        );
        let actual = rasterize(scene, size, &settings);
        let palette = settings.appearance.palette();
        let mut expected = Scene::new(view.assets.clone());
        expected.frame(
            Rect {
                x: 16.,
                y: 16.,
                width: 496.,
                height: plan.height - 32.,
            },
            12.,
            palette.raised,
            palette.border,
            1.,
        );
        expected.label(
            fl!("session-key-identity"),
            Rect {
                x: 32.,
                y: 26.,
                width: 464.,
                height: 20.,
            },
            13.,
            palette.muted,
            false,
            Weight::Medium,
        );
        // Render an independent Iced monospace text node as the expected hex
        // row, so a regression to the proportional label path is visible.
        for (index, group) in groups.iter().enumerate() {
            expected.place(
                iced::widget::container(
                    iced::widget::text(*group)
                        .font(iced::Font {
                            family: iced::font::Family::Monospace,
                            weight: Weight::Medium,
                            ..iced::Font::DEFAULT
                        })
                        .size(14.5)
                        .color(canopa_ui::color(palette.text))
                        .shaping(iced::advanced::text::Shaping::Advanced),
                )
                .center_y(iced::Length::Fill),
                Rect {
                    x: 32.,
                    y: 134. + 22. * index as f32,
                    width: 464.,
                    height: 22.,
                },
            );
        }
        let expected = rasterize(expected, size, &settings);
        for (start, end) in [(26, 46), (134, 178)] {
            for y in start..end {
                let range = ((y * 528 + 32) * 4)..((y * 528 + 496) * 4);
                assert_eq!(
                    &actual.data()[range.clone()],
                    &expected.data()[range],
                    "fingerprint card at row {y}"
                );
            }
        }
    }
}
