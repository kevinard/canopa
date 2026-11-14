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
use super::raster::{Emblem, blend, emblem, framed, gradient_disc, mix, rounded, shadow};
use super::{Control, ControlKind, SessionView, wrap};
use crate::{layout::Rect, text::Style};
use canopa_theme::{Appearance, ColorRole, ColorScheme, Palette, Rgb};
use cosmic_text::Weight;
use image::RgbImage;

pub const DIALOG_SIZE: (u32, u32) = (560, 416);
pub const AUTHORIZATION_DIALOG_SIZE: (u32, u32) = (560, 392);
pub const DIALOG_MARGIN: f32 = 16.;
/// Dialog windows fit their content between these logical heights.
pub const DIALOG_MIN_HEIGHT: u32 = 280;
pub const DIALOG_MAX_HEIGHT: u32 = 560;

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

/// The request card's second column in a policy dialog.
#[derive(Clone, Copy)]
enum RequestContext<'a> {
    RequestedBy(&'a str),
    PolicySource(&'a str),
    RequestedFor(&'a str),
}

impl RequestContext<'_> {
    fn title(self) -> String {
        match self {
            Self::RequestedBy(_) => fl!("session-requested-by"),
            Self::PolicySource(_) => fl!("session-policy-source"),
            Self::RequestedFor(_) => fl!("session-requested-for"),
        }
    }
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

    pub(super) fn size(&self) -> (f32, f32) {
        (self.width, self.height.max(self.height_reserve))
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

    pub(super) fn draw(
        &self,
        view: &SessionView,
        settings: &crate::settings::Settings,
        image: &mut RgbImage,
        mut bounds: Rect,
        scale: f32,
    ) -> Vec<(usize, Rect)> {
        let appearance = &settings.appearance;
        let palette = appearance.palette();
        let light = appearance.color_scheme == ColorScheme::Light;
        bounds.height = self.height * scale;
        if self.mode == Mode::Sheet {
            shadow(
                image,
                bounds,
                24. * scale,
                scale,
                if light { 0.07 } else { 0.22 },
            );
            let edge = mix(
                palette.surface,
                palette.border,
                if light { 0.42 } else { 0.5 },
            );
            framed(image, bounds, 24. * scale, scale, palette.surface, edge);
        }
        let padding = match self.mode {
            Mode::Form => 0.,
            Mode::Sheet => SHEET_PADDING,
            Mode::Dialog => DIALOG_PADDING,
        };
        let left = bounds.x + padding * scale;
        let width = bounds.width - 2. * padding * scale;
        let rtl = canopa_i18n::direction().is_rtl();
        let mut hits = Vec::new();
        let (title, subtitle) = self.titles(view);
        // Dialogs share spare height evenly above and below their body.
        let footer_top = (self.height_reserve.max(self.height) - DIALOG_PADDING - BUTTON)
            .max(self.height + 26. - DIALOG_PADDING);
        let spare = if self.mode == Mode::Dialog {
            ((footer_top - (self.height - DIALOG_PADDING) - 24.) / 3.).clamp(0., 14.)
        } else {
            0.
        };
        for (index, (offset, row)) in self.rows.iter().enumerate() {
            let shift = if index > 0 { spare } else { 0. };
            let top = bounds.y + (offset + shift) * scale;
            let mut paint = Paint {
                view,
                image,
                appearance,
                palette,
                scale,
                left,
                top,
                width,
                rtl,
            };
            match row {
                Content::Identity => paint.identity(
                    title,
                    subtitle,
                    self.avatar || self.mode == Mode::Form,
                    self.username,
                ),
                Content::Heading => paint.heading(self.panel, title, subtitle, self.username),
                Content::Request(request) => paint.request(request),
                Content::Divider => {
                    let line = Rect {
                        x: left,
                        y: top,
                        width,
                        height: scale,
                    };
                    let color = mix(palette.surface, palette.border, 0.42);
                    paint.fill(line, 0., color, 1.);
                }
                Content::Copy(lines, tone) => paint.copy(lines, *tone, self.mode == Mode::Form),
                Content::Field {
                    label,
                    placeholder,
                    submit,
                } => {
                    if self.mode == Mode::Form {
                        paint.form_field(label, placeholder, *submit, &mut hits)
                    } else {
                        paint.dialog_field(label, placeholder, &mut hits)
                    }
                }
                Content::Pill(control) => paint.pill(control, &mut hits),
                Content::Links(controls) => paint.links(controls, &mut hits),
                Content::Accounts(controls) => paint.accounts(controls, &mut hits),
                Content::Buttons(controls, treatment) => {
                    paint.buttons(controls, *treatment, self.panel, &mut hits)
                }
                Content::System => {
                    let row = Rect {
                        x: left,
                        y: top,
                        width,
                        height: CHIP * scale,
                    };
                    system(
                        view,
                        settings,
                        paint.image,
                        &self.system,
                        row,
                        scale,
                        true,
                        &mut hits,
                    );
                }
            }
        }
        if self.mode == Mode::Dialog
            && (!self.footer.is_empty() || self.footer_disclosure.is_some())
        {
            let top = bounds.y + footer_top * scale;
            let mut paint = Paint {
                view,
                image,
                appearance,
                palette,
                scale,
                left,
                top,
                width,
                rtl,
            };
            paint.footer(&self.footer, self.footer_disclosure, &mut hits);
        }
        if !self.system_inline && !self.system.is_empty() {
            let (screen_width, screen_height) = image.dimensions();
            let margin = settings.layout.margin * scale;
            let row = Rect {
                x: margin,
                y: screen_height as f32 - margin - CHIP * scale,
                width: screen_width as f32 - 2. * margin,
                height: CHIP * scale,
            };
            system(
                view,
                settings,
                image,
                &self.system,
                row,
                scale,
                false,
                &mut hits,
            );
        }
        hits
    }
}

/// Chips for the desktop and keyboard, and a round power button, either at the
/// bottom corners of `row` (spread) or centred in it.
#[allow(clippy::too_many_arguments)]
fn system(
    view: &SessionView,
    settings: &crate::settings::Settings,
    image: &mut RgbImage,
    items: &[SystemItem],
    row: Rect,
    s: f32,
    centred: bool,
    hits: &mut Vec<(usize, Rect)>,
) {
    let appearance = &settings.appearance;
    let palette = appearance.palette();
    let light = appearance.color_scheme == ColorScheme::Light;
    let rtl = canopa_i18n::direction().is_rtl();
    let focus = appearance.role(ColorRole::Focus);
    let size = 14.;
    // (control, icon, label, shortcut, trailing chevron, width)
    struct Chip<'c> {
        control: Option<&'c Control>,
        icon: Emblem,
        label: String,
        shortcut: Option<&'static str>,
        menu: bool,
        round: bool,
        width: f32,
        start: bool,
        level: Option<f32>,
    }
    let mut chips: Vec<Chip> = items
        .iter()
        .map(|item| {
            let (control, icon, label, shortcut, menu, round, start) = match item {
                SystemItem::Desktop(control) => (
                    Some(*control),
                    Emblem::Display,
                    control.label.clone(),
                    None,
                    true,
                    false,
                    true,
                ),
                SystemItem::Keyboard(control) => (
                    *control,
                    Emblem::Keyboard,
                    fl!("session-keyboard-layout", layout = view.layout.as_str()),
                    None,
                    false,
                    false,
                    false,
                ),
                SystemItem::Power(control) => (
                    Some(*control),
                    Emblem::Power,
                    String::new(),
                    None,
                    false,
                    true,
                    false,
                ),
                SystemItem::Status(icon, text, _) => {
                    (None, *icon, text.clone(), None, false, false, false)
                }
            };
            let mut text = view.text_width(&label, size * s).min(180. * s);
            if let Some(shortcut) = shortcut {
                text += 8. * s + view.text_width(shortcut, 12. * s);
            }
            let width = if round {
                CHIP * s
            } else {
                (14. + 20. + 8.) * s + text + if menu { 34. } else { 16. } * s
            };
            Chip {
                control,
                icon,
                label,
                shortcut,
                menu,
                round,
                width,
                start,
                level: match item {
                    SystemItem::Status(_, _, level) => *level,
                    _ => None,
                },
            }
        })
        .collect();
    let gap = 10. * s;
    let place = |chips: &mut [Chip], from_start: bool, centred: bool| -> Vec<f32> {
        let total: f32 =
            chips.iter().map(|c| c.width).sum::<f32>() + gap * chips.len().saturating_sub(1) as f32;
        let mut x = if centred {
            row.x + (row.width - total) / 2.
        } else if from_start {
            row.x
        } else {
            row.x + row.width - total
        };
        chips
            .iter()
            .map(|chip| {
                let at = x;
                x += chip.width + gap;
                at
            })
            .collect()
    };
    let mut placed = Vec::new();
    if centred {
        let xs = place(&mut chips, true, true);
        placed.extend(xs.into_iter().zip(0..));
    } else {
        let (mut start, mut end): (Vec<usize>, Vec<usize>) =
            (0..chips.len()).partition(|&i| chips[i].start);
        if rtl {
            std::mem::swap(&mut start, &mut end);
            end.reverse();
            start.reverse();
        }
        for (indices, from_start) in [(start, true), (end, false)] {
            let widths: Vec<f32> = indices.iter().map(|&i| chips[i].width).collect();
            let total: f32 =
                widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32;
            let mut x = if from_start {
                row.x
            } else {
                row.x + row.width - total
            };
            for (&i, width) in indices.iter().zip(widths) {
                placed.push((x, i));
                x += width + gap;
            }
        }
    }
    for (x, index) in placed {
        let chip = &chips[index];
        let rect = Rect {
            x,
            y: row.y,
            width: chip.width,
            height: CHIP * s,
        };
        let selected = chip.control.is_some_and(|c| c.selected);
        let fill = if light {
            palette.surface
        } else {
            mix(palette.surface, palette.raised, 0.35)
        };
        let border = if selected {
            focus
        } else {
            mix(fill, palette.border, if light { 0.5 } else { 0.58 })
        };
        rounded(
            image,
            rect,
            CHIP / 2. * s,
            border,
            if light { 0.9 } else { 0.85 },
        );
        let inner = Rect {
            x: rect.x + if selected { 1.5 } else { 1. } * s,
            y: rect.y + if selected { 1.5 } else { 1. } * s,
            width: rect.width - if selected { 3. } else { 2. } * s,
            height: rect.height - if selected { 3. } else { 2. } * s,
        };
        rounded(
            image,
            inner,
            CHIP / 2. * s,
            fill,
            if light { 0.96 } else { 0.94 },
        );
        let ink = palette.text;
        if chip.round {
            emblem(
                image,
                (
                    rect.x + (CHIP / 2. - 10.) * s,
                    rect.y + (CHIP / 2. - 10.) * s,
                ),
                (20. / 24.) * s,
                chip.icon,
                ink,
            );
        } else {
            let (icon_x, text_x, chevron_x) = if rtl {
                (
                    rect.x + rect.width - (14. + 20.) * s,
                    rect.x + rect.width - (14. + 20. + 8.) * s,
                    rect.x + 8. * s,
                )
            } else {
                (
                    rect.x + 14. * s,
                    rect.x + (14. + 20. + 8.) * s,
                    rect.x + rect.width - 29. * s,
                )
            };
            let glyph = mix(palette.muted, palette.text, 0.4);
            emblem(
                image,
                (icon_x, rect.y + (CHIP / 2. - 10.) * s),
                (20. / 24.) * s,
                chip.icon,
                glyph,
            );
            if let Some(level) = chip.level {
                // The battery's charge fills its outline.
                let unit = (20. / 24.) * s;
                let (start, top) = (
                    icon_x + 5. * unit,
                    rect.y + (CHIP / 2. - 10.) * s + 9. * unit,
                );
                let width = 11. * unit * level.clamp(0.05, 1.);
                let x = if rtl {
                    start + 11. * unit - width
                } else {
                    start
                };
                rounded(
                    image,
                    Rect {
                        x,
                        y: top,
                        width,
                        height: 6. * unit,
                    },
                    unit,
                    glyph,
                    1.,
                );
            }
            let label_width = view.text_width(&chip.label, size * s).min(180. * s);
            view.text_renderer.borrow_mut().draw_styled(
                image,
                &chip.label,
                (text_x, rect.y + (CHIP / 2. + 5.) * s),
                size * s,
                180. * s,
                ink,
                None,
                if rtl {
                    Style::Trailing(Weight::MEDIUM)
                } else {
                    Style::Leading(Weight::MEDIUM)
                },
            );
            if let Some(shortcut) = chip.shortcut {
                let x = if rtl {
                    text_x - label_width - 8. * s
                } else {
                    text_x + label_width + 8. * s
                };
                view.text_renderer.borrow_mut().draw_styled(
                    image,
                    shortcut,
                    (x, rect.y + (CHIP / 2. + 4.5) * s),
                    12. * s,
                    40. * s,
                    palette.muted,
                    None,
                    if rtl {
                        Style::Trailing(Weight::MEDIUM)
                    } else {
                        Style::Leading(Weight::MEDIUM)
                    },
                );
            }
            if chip.menu {
                emblem(
                    image,
                    (chevron_x, rect.y + (CHIP / 2. - 7.) * s),
                    (14. / 24.) * s,
                    Emblem::ChevronDown,
                    palette.muted,
                );
            }
        }
        if let Some(control) = chip.control {
            hits.push((control.id, rect));
        }
    }
}

struct Paint<'a> {
    view: &'a SessionView,
    image: &'a mut RgbImage,
    appearance: &'a Appearance,
    palette: Palette,
    scale: f32,
    left: f32,
    top: f32,
    width: f32,
    /// Mirror the panel for right-to-left languages.
    rtl: bool,
}

impl Paint<'_> {
    /// Mirror an absolute x coordinate within the panel when right to left.
    fn flip(&self, x: f32) -> f32 {
        if self.rtl {
            2. * self.left + self.width - x
        } else {
            x
        }
    }

    /// `rect`, laid out left to right, at its place in the current direction.
    fn mirror(&self, rect: Rect) -> Rect {
        if self.rtl {
            Rect {
                x: self.flip(rect.x) - rect.width,
                ..rect
            }
        } else {
            rect
        }
    }

    fn light(&self) -> bool {
        self.appearance.color_scheme == ColorScheme::Light
    }

    fn focus(&self) -> Rgb {
        self.appearance.role(ColorRole::Focus)
    }

    fn fill(&mut self, rect: Rect, radius: f32, color: Rgb, alpha: f32) {
        let rect = self.mirror(rect);
        rounded(self.image, rect, radius, color, alpha);
    }

    fn frame(&mut self, rect: Rect, radius: f32, border: f32, fill: Rgb, edge: Rgb) {
        let rect = self.mirror(rect);
        framed(self.image, rect, radius, border, fill, edge);
    }

    fn hit(&self, hits: &mut Vec<(usize, Rect)>, id: usize, rect: Rect) {
        hits.push((id, self.mirror(rect)));
    }

    /// A 24-unit emblem; directional glyphs point the other way when mirrored.
    fn icon(&mut self, (x, y): (f32, f32), scale: f32, kind: Emblem, color: Rgb) {
        let (x, kind) = if self.rtl {
            let kind = match kind {
                Emblem::Chevron => Emblem::ChevronBack,
                Emblem::ChevronBack => Emblem::Chevron,
                Emblem::Arrow => Emblem::ArrowBack,
                Emblem::ArrowBack => Emblem::Arrow,
                other => other,
            };
            (self.flip(x) - 24. * scale, kind)
        } else {
            (x, kind)
        };
        emblem(self.image, (x, y), scale, kind, color);
    }

    #[allow(clippy::too_many_arguments)]
    fn styled(
        &mut self,
        text: &str,
        (x, y): (f32, f32),
        size: f32,
        width: f32,
        color: Rgb,
        selection: Option<(std::ops::Range<usize>, usize, Rgb)>,
        style: Style,
    ) {
        let (x, style) = if self.rtl {
            (self.flip(x), style.mirrored())
        } else {
            (x, style)
        };
        self.view.text_renderer.borrow_mut().draw_styled(
            self.image,
            text,
            (x, y),
            size,
            width,
            color,
            selection,
            style,
        );
    }

    fn clean(value: &str) -> String {
        value
            .chars()
            .filter(|c| !c.is_control())
            .take(256)
            .collect()
    }

    fn label(
        &mut self,
        value: &str,
        (x, y): (f32, f32),
        size: f32,
        width: f32,
        color: Rgb,
        weight: Weight,
    ) {
        let text = Self::clean(value);
        self.styled(
            &text,
            (self.left + x * self.scale, self.top + y * self.scale),
            size * self.scale,
            width,
            color,
            None,
            Style::Leading(weight),
        );
    }

    fn centred(&mut self, value: &str, y: f32, size: f32, color: Rgb, weight: Weight) {
        let text = Self::clean(value);
        self.styled(
            &text,
            (self.left + self.width / 2., self.top + y * self.scale),
            size * self.scale,
            self.width,
            color,
            None,
            Style::Centered(weight),
        );
    }

    /// The tone that reads on the accent surface: the accent, or text.
    fn tint(&self, background: Rgb) -> Rgb {
        if self.palette.accent.contrast(background) >= 4.5 {
            self.palette.accent
        } else {
            self.palette.text
        }
    }

    /// Initials from the first and last words of a name.
    fn initials(name: &str) -> String {
        let mut words = name
            .split_whitespace()
            .filter_map(|word| word.chars().find(|c| c.is_alphanumeric()));
        let first = words.next();
        let last = words.next_back();
        first
            .into_iter()
            .chain(last)
            .flat_map(char::to_uppercase)
            .collect()
    }

    /// `named`: the title is the person's name, so it can supply initials.
    fn identity(&mut self, title: &str, subtitle: &str, show_avatar: bool, named: bool) {
        let s = self.scale;
        let circle = Rect {
            x: self.left + (self.width - AVATAR * s) / 2.,
            y: self.top,
            width: AVATAR * s,
            height: AVATAR * s,
        };
        if !(show_avatar && self.avatar(circle, circle.width / 2.)) {
            // A flat disc in the person's colour; a generic person keeps the accent.
            let (top, tint) = if named || self.view.avatar_initials.is_some() {
                self.person_tone(title)
            } else {
                let surface = self.appearance.role(ColorRole::AccentSurface);
                let light = self.light();
                let tone = mix(surface, self.palette.accent, if light { 0.16 } else { 0.3 });
                (tone, self.tint(tone))
            };
            gradient_disc(self.image, circle, top, top);
            let initials = self
                .view
                .avatar_initials
                .clone()
                .filter(|_| show_avatar)
                .or_else(|| named.then(|| Self::initials(title)))
                .filter(|initials| !initials.is_empty());
            if let Some(initials) = initials {
                self.view.text_renderer.borrow_mut().draw_styled(
                    self.image,
                    &initials,
                    (circle.x + circle.width / 2., circle.y + 54. * s),
                    28. * s,
                    circle.width - 16. * s,
                    tint,
                    None,
                    Style::Centered(Weight::MEDIUM),
                );
            } else {
                emblem(
                    self.image,
                    (circle.x + 20. * s, circle.y + 20. * s),
                    2. * s,
                    Emblem::Person,
                    tint,
                );
            }
        }
        self.centred(
            title,
            AVATAR + 16. + 24.,
            26.,
            self.palette.text,
            Weight::SEMIBOLD,
        );
        if !subtitle.is_empty() {
            let ink = mix(self.palette.muted, self.palette.text, 0.5);
            self.centred(subtitle, AVATAR + 16. + 30. + 21., 15., ink, Weight::NORMAL);
        }
    }

    fn heading(
        &mut self,
        panel: &AuthenticationPanel,
        title: &str,
        subtitle: &str,
        username: bool,
    ) {
        let s = self.scale;
        let tile = Rect {
            x: self.left,
            y: self.top,
            width: 46. * s,
            height: 46. * s,
        };
        let surface = self.appearance.role(ColorRole::AccentSurface);
        let tint = self.tint(surface);
        let avatar = username && !panel.list_actions && !panel.dialog;
        if !(avatar && self.avatar(tile, 23. * s)) {
            self.fill(tile, if avatar { 23. } else { 13. } * s, surface, 1.);
            let initials = self
                .view
                .avatar_initials
                .clone()
                .filter(|_| avatar)
                .or_else(|| avatar.then(|| Self::initials(title)))
                .filter(|initials| !initials.is_empty());
            if let Some(initials) = initials {
                self.styled(
                    &initials,
                    (tile.x + 23. * s, tile.y + 29.5 * s),
                    17. * s,
                    tile.width - 8. * s,
                    tint,
                    None,
                    Style::Centered(Weight::MEDIUM),
                );
            } else {
                self.icon(
                    (tile.x + 9. * s, tile.y + 9. * s),
                    (28. / 24.) * s,
                    panel.emblem,
                    tint,
                );
            }
        }
        let width = self.width - 62. * s;
        let text = self.palette.text;
        if subtitle.is_empty() {
            self.label(title, (62., 30.), 21., width, text, Weight::SEMIBOLD);
        } else {
            self.label(title, (62., 20.), 20., width, text, Weight::SEMIBOLD);
            let secondary = self.secondary();
            self.label(subtitle, (62., 40.), 14., width, secondary, Weight::NORMAL);
        }
    }

    fn avatar(&mut self, tile: Rect, radius: f32) -> bool {
        let Some(avatar) = self
            .view
            .user_avatar
            .as_deref()
            .or(self.view.avatar.as_ref())
        else {
            return false;
        };
        let tile = self.mirror(tile);
        let side = tile.width.round().max(1.) as u32;
        let avatar =
            image::imageops::resize(avatar, side, side, image::imageops::FilterType::Triangle);
        let inset = tile.width / 2. - radius;
        for (x, y, pixel) in avatar.enumerate_pixels() {
            let dx = (x as f32 + 0.5 - tile.width / 2.).abs();
            let dy = (y as f32 + 0.5 - tile.height / 2.).abs();
            let corner = (dx - inset).max(0.).hypot((dy - inset).max(0.));
            if let Some(target) = self
                .image
                .get_pixel_mut_checked(tile.x as u32 + x, tile.y as u32 + y)
            {
                blend(
                    target,
                    Rgb(pixel[0], pixel[1], pixel[2]),
                    (radius + 0.5 - corner).clamp(0., 1.) * f32::from(pixel[3]) / 255.,
                );
            }
        }
        true
    }

    fn copy(&mut self, lines: &[String], tone: Tone, centred: bool) {
        let palette = self.palette;
        let ink = match tone {
            Tone::Notice if self.view.failed => palette.danger,
            Tone::Caps => palette.warning,
            Tone::Body => mix(palette.muted, palette.text, 0.55),
            _ => palette.muted,
        };
        let weight = if tone == Tone::Muted || tone == Tone::Body {
            Weight::NORMAL
        } else {
            Weight::MEDIUM
        };
        for (i, line) in lines.iter().enumerate() {
            if centred {
                self.centred(line, 15. + i as f32 * 21., 14.5, ink, weight);
            } else {
                let width = self.width;
                self.label(line, (0., 16. + i as f32 * 22.), 15., width, ink, weight);
            }
        }
    }

    /// The caret and placeholder of an empty field, or its masked or echoed text.
    fn field_text(&mut self, placeholder: &str, x: f32, baseline: f32, width: f32) {
        let s = self.scale;
        let focus = self.focus();
        let prompt = self
            .view
            .prompt
            .as_ref()
            .expect("visible field has a prompt");
        let (input, selection, cursor) = self
            .view
            .secret
            .presentation(prompt.echo || self.view.revealed(), 48);
        if self.view.secret.is_empty() {
            let caret = Rect {
                x,
                y: self.top + (baseline - 16.5) * s,
                width: 1.5 * s,
                height: 21. * s,
            };
            self.fill(caret, 0.75 * s, focus, 1.);
            let muted = mix(self.palette.muted, self.palette.background, 0.12);
            let placeholder = Self::clean(placeholder);
            self.styled(
                &placeholder,
                (x + 7. * s, self.top + baseline * s),
                16. * s,
                width - 7. * s,
                muted,
                None,
                Style::Leading(Weight::NORMAL),
            );
        } else {
            let text = self.palette.text;
            self.styled(
                input.as_ref(),
                (x, self.top + baseline * s),
                16. * s,
                width,
                text,
                Some((selection, cursor, focus)),
                Style::Leading(Weight::NORMAL),
            );
        }
    }

    fn form_field(
        &mut self,
        label: &[String],
        placeholder: &str,
        submit: Option<&Control>,
        hits: &mut Vec<(usize, Rect)>,
    ) {
        let s = self.scale;
        let muted = self.palette.muted;
        for (i, line) in label.iter().enumerate() {
            self.centred(line, 14. + i as f32 * 21., 14.5, muted, Weight::NORMAL);
        }
        let offset = if label.is_empty() {
            0.
        } else {
            21. * label.len() as f32 + 10.
        };
        let field = Rect {
            x: self.left,
            y: self.top + offset * s,
            width: self.width,
            height: FIELD * s,
        };
        let light = self.light();
        shadow(
            self.image,
            self.mirror(field),
            FIELD / 2. * s,
            s,
            if light { 0.05 } else { 0.16 },
        );
        let fill = if light {
            self.palette.surface
        } else {
            mix(self.palette.background, self.palette.surface, 0.55)
        };
        let focus = self.focus();
        self.frame(field, FIELD / 2. * s, 1.5 * s, fill, focus);
        // A name field says so with a leading glyph; a secret field is plain.
        let echo = self.view.prompt.as_ref().is_some_and(|prompt| prompt.echo);
        if echo {
            let muted = self.palette.muted;
            self.icon(
                (field.x + 16. * s, field.y + 14. * s),
                0.83 * s,
                Emblem::Person,
                muted,
            );
        }
        let inset = if echo { 46. } else { 22. } * s;
        let reveal = self.view.can_reveal();
        let trailing =
            if submit.is_some() { 62. * s } else { 20. * s } + if reveal { 36. * s } else { 0. };
        if reveal {
            let end = field.x + field.width - if submit.is_some() { 58. } else { 14. } * s;
            self.reveal_button(end, field.y + field.height / 2., hits);
        }
        let top = self.top;
        self.top = field.y;
        self.field_text(
            placeholder,
            field.x + inset,
            32.5,
            field.width - inset - trailing,
        );
        self.top = top;
        if let Some(control) = submit {
            let button = Rect {
                x: field.x + field.width - 46. * s,
                y: field.y + 6. * s,
                width: 40. * s,
                height: 40. * s,
            };
            let accent = self.palette.accent;
            if control.selected {
                let ring = Rect {
                    x: button.x - 3. * s,
                    y: button.y - 3. * s,
                    width: button.width + 6. * s,
                    height: button.height + 6. * s,
                };
                self.fill(ring, ring.width / 2., focus, 1.);
                self.fill(
                    Rect {
                        x: ring.x + 1.5 * s,
                        y: ring.y + 1.5 * s,
                        width: ring.width - 3. * s,
                        height: ring.height - 3. * s,
                    },
                    ring.width / 2.,
                    fill,
                    1.,
                );
            }
            let ink = self.appearance.role(ColorRole::OnAccent);
            self.fill(button, button.width / 2., accent, 1.);
            // A 24-unit arrow centred in the button.
            self.icon(
                (
                    button.x + button.width / 2. - 12. * s,
                    button.y + button.height / 2. - 12. * s,
                ),
                s,
                Emblem::Arrow,
                ink,
            );
            self.hit(hits, control.id, button);
        }
    }

    /// The eye button that ends at `end`, vertically centred on `middle`.
    fn reveal_button(&mut self, end: f32, middle: f32, hits: &mut Vec<(usize, Rect)>) {
        let s = self.scale;
        let rect = Rect {
            x: end - 34. * s,
            y: middle - 17. * s,
            width: 34. * s,
            height: 34. * s,
        };
        let ink = if self.view.revealed() {
            self.focus()
        } else {
            mix(self.palette.muted, self.palette.text, 0.45)
        };
        self.icon(
            (rect.x + 5. * s, rect.y + 5. * s),
            s,
            if self.view.revealed() {
                Emblem::EyeOff
            } else {
                Emblem::Eye
            },
            ink,
        );
        self.hit(hits, crate::input::REVEAL_CONTROL, rect);
    }

    fn dialog_field(&mut self, label: &[String], placeholder: &str, hits: &mut Vec<(usize, Rect)>) {
        let s = self.scale;
        let text = self.palette.text;
        for (i, line) in label.iter().enumerate() {
            let width = self.width;
            self.label(
                line,
                (0., 14. + i as f32 * 21.),
                14.,
                width,
                text,
                Weight::MEDIUM,
            );
        }
        let offset = if label.is_empty() {
            0.
        } else {
            21. * label.len() as f32 + 8.
        };
        let field = Rect {
            x: self.left,
            y: self.top + offset * s,
            width: self.width,
            height: BUTTON * s,
        };
        let light = self.light();
        let fill = if light {
            mix(self.palette.surface, self.palette.background, 0.45)
        } else {
            mix(self.palette.background, self.palette.surface, 0.3)
        };
        // A slimmer ring than the full-screen field: the primary button leads.
        let focus = self.focus();
        self.frame(field, BUTTON / 2. * s, 1.25 * s, fill, focus);
        let reveal = self.view.can_reveal();
        if reveal {
            self.reveal_button(
                field.x + field.width - 6. * s,
                field.y + field.height / 2.,
                hits,
            );
        }
        let top = self.top;
        self.top = field.y;
        let width = field.width - if reveal { 54. } else { 28. } * s;
        self.field_text(placeholder, field.x + 18. * s, 27.5, width - 4. * s);
        self.top = top;
    }

    fn pill(&mut self, control: &Control, hits: &mut Vec<(usize, Rect)>) {
        let s = self.scale;
        let rect = Rect {
            x: self.left,
            y: self.top,
            width: self.width,
            height: FIELD * s,
        };
        let primary = self.view.primary_control == Some(control.id) && !self.view.busy;
        let (fill, ink) = if primary {
            (
                self.palette.accent,
                self.appearance.role(ColorRole::OnAccent),
            )
        } else if self.light() {
            (
                mix(self.palette.surface, self.palette.raised, 0.6),
                self.palette.muted,
            )
        } else {
            (
                mix(self.palette.background, self.palette.surface, 0.7),
                self.palette.muted,
            )
        };
        let edge = if control.selected {
            self.focus()
        } else if primary {
            fill
        } else {
            mix(fill, self.palette.border, 0.45)
        };
        self.frame(
            rect,
            FIELD / 2. * s,
            if control.selected { 1.5 } else { 1. } * s,
            fill,
            edge,
        );
        let label = Self::clean(&control.label);
        self.styled(
            &label,
            (rect.x + rect.width / 2., rect.y + 29.5 * s),
            15. * s,
            rect.width - 32. * s,
            ink,
            None,
            Style::Centered(if primary {
                Weight::SEMIBOLD
            } else {
                Weight::MEDIUM
            }),
        );
        self.hit(hits, control.id, rect);
    }

    /// Account rows: avatar, name and username; the selected row is the
    /// default and shows the submit arrow.
    fn accounts(&mut self, controls: &[&Control], hits: &mut Vec<(usize, Rect)>) {
        let s = self.scale;
        let light = self.light();
        let focus = self.focus();
        for (index, control) in controls.iter().enumerate() {
            let ControlKind::Account {
                name,
                detail,
                initials,
            } = &control.kind
            else {
                continue;
            };
            let rect = Rect {
                x: self.left,
                y: self.top + index as f32 * (ACCOUNT_ROW + 10.) * s,
                width: self.width,
                height: ACCOUNT_ROW * s,
            };
            let selected = control.selected;
            let fill = if selected {
                if light {
                    self.palette.surface
                } else {
                    mix(self.palette.surface, self.palette.raised, 0.5)
                }
            } else if light {
                mix(self.palette.surface, self.palette.background, 0.2)
            } else {
                mix(self.palette.background, self.palette.surface, 0.85)
            };
            let edge = if selected {
                focus
            } else {
                mix(fill, self.palette.border, 0.5)
            };
            let drawn = self.mirror(rect);
            shadow(
                self.image,
                drawn,
                18. * s,
                s,
                if selected {
                    if light { 0.06 } else { 0.2 }
                } else {
                    0.
                },
            );
            let border = if selected { 1.5 } else { 1. } * s;
            let alpha = if selected { 1. } else { 0.94 };
            rounded(self.image, drawn, 18. * s, edge, alpha);
            rounded(
                self.image,
                Rect {
                    x: drawn.x + border,
                    y: drawn.y + border,
                    width: drawn.width - 2. * border,
                    height: drawn.height - 2. * border,
                },
                18. * s - border,
                fill,
                alpha,
            );
            // The avatar: the consumer's picture for the selected account, else initials.
            let disc = Rect {
                x: rect.x + 12. * s,
                y: rect.y + 12. * s,
                width: 52. * s,
                height: 52. * s,
            };
            if !(selected && self.avatar(disc, 26. * s)) {
                let (tone, ink) = self.person_tone(name);
                self.fill(disc, 26. * s, tone, 1.);
                if initials.is_empty() {
                    self.icon(
                        (disc.x + 12. * s, disc.y + 12. * s),
                        (28. / 24.) * s,
                        Emblem::Person,
                        ink,
                    );
                } else {
                    self.styled(
                        initials,
                        (disc.x + 26. * s, disc.y + 32.5 * s),
                        18. * s,
                        disc.width - 8. * s,
                        ink,
                        None,
                        Style::Centered(Weight::SEMIBOLD),
                    );
                }
            }
            let text_x = rect.x + 78. * s;
            let available = rect.width - 78. * s - 60. * s;
            let text = self.palette.text;
            let secondary = self.secondary();
            let name = Self::clean(name);
            if let Some(detail) = detail {
                self.styled(
                    &name,
                    (text_x, rect.y + 34. * s),
                    16.5 * s,
                    available,
                    text,
                    None,
                    Style::Leading(Weight::SEMIBOLD),
                );
                let detail = Self::clean(detail);
                self.styled(
                    &detail,
                    (text_x, rect.y + 55. * s),
                    14. * s,
                    available,
                    secondary,
                    None,
                    Style::Leading(Weight::NORMAL),
                );
            } else {
                self.styled(
                    &name,
                    (text_x, rect.y + 44. * s),
                    16.5 * s,
                    available,
                    text,
                    None,
                    Style::Leading(Weight::SEMIBOLD),
                );
            }
            // Every row signs in the same way; selection only marks Enter's target.
            let ink = if selected { focus } else { self.secondary() };
            self.icon(
                (rect.x + rect.width - 44. * s, rect.y + 28. * s),
                (20. / 24.) * s,
                Emblem::Chevron,
                ink,
            );
            self.hit(hits, control.id, rect);
        }
    }

    fn links(&mut self, controls: &[&Control], hits: &mut Vec<(usize, Rect)>) {
        let s = self.scale;
        let size = 15.;
        let gap = 12. * s;
        let items: Vec<(String, f32)> = controls
            .iter()
            .map(|control| {
                let label = match &control.kind {
                    ControlKind::AccountChooser { text } => text.clone(),
                    _ => control.label.clone(),
                };
                let width = 18. * s + 7. * s + self.view.text_width(&label, size * s);
                (label, width)
            })
            .collect();
        let total: f32 =
            items.iter().map(|(_, w)| w + 24. * s).sum::<f32>() + gap * (items.len() - 1) as f32;
        let mut x = self.left + (self.width - total) / 2.;
        let focus = self.focus();
        let ink = mix(self.palette.muted, self.palette.text, 0.6);
        for (control, (label, width)) in controls.iter().zip(items) {
            let rect = Rect {
                x,
                y: self.top,
                width: width + 24. * s,
                height: 36. * s,
            };
            // A text action; keyboard focus draws its outline.
            if control.selected {
                let surface = self.appearance.role(ColorRole::AccentSurface);
                self.frame(rect, 18. * s, 1.5 * s, surface, focus);
            }
            self.icon(
                (rect.x + 12. * s, rect.y + 9. * s),
                0.75 * s,
                Emblem::Accounts,
                ink,
            );
            self.styled(
                &label,
                (rect.x + 37. * s, rect.y + 23.5 * s),
                size * s,
                width,
                ink,
                None,
                Style::Leading(Weight::MEDIUM),
            );
            self.hit(hits, control.id, rect);
            x += rect.width + gap;
        }
    }

    fn request(&mut self, request: &Request) {
        let s = self.scale;
        let height = request.height() * s;
        let light = self.light();
        let card = if light {
            mix(self.palette.surface, self.palette.raised, 0.75)
        } else {
            mix(self.palette.surface, self.palette.raised, 0.95)
        };
        let edge = mix(card, self.palette.border, if light { 0.45 } else { 0.42 });
        let rect = Rect {
            x: self.left,
            y: self.top,
            width: self.width,
            height,
        };
        self.frame(rect, 12. * s, s, card, edge);
        let divider = mix(card, self.palette.border, 0.45);
        let muted = self.palette.muted;
        let text = self.palette.text;
        if let Some(fingerprint) = (!request.policy)
            .then(|| fingerprint_groups(&request.lines))
            .flatten()
        {
            self.key_identity(request.subject, fingerprint, divider);
            return;
        }
        let mut y = 0.;
        if request.subject.is_some() || !request.lines.is_empty() {
            let mut baseline = 27.;
            let width = self.width - 32. * s;
            if let Some(subject) = request.subject {
                self.label(
                    subject,
                    (16., baseline),
                    15.5,
                    width,
                    text,
                    Weight::SEMIBOLD,
                );
                baseline += 23.;
            }
            for line in &request.lines {
                let groups: Vec<_> = line.split_ascii_whitespace().collect();
                if !request.policy && hex_groups(&groups) {
                    self.request_code(line, 16., baseline, width);
                } else {
                    let ink = mix(muted, text, if request.policy { 0.8 } else { 0.55 });
                    self.label(line, (16., baseline), 14.5, width, ink, Weight::NORMAL);
                }
                baseline += 21.;
            }
            y = baseline - 7.;
        }
        let Some(name) = request.account else {
            return;
        };
        if y > 0. {
            self.fill(
                Rect {
                    x: self.left + 16. * s,
                    y: self.top + (y + 4.) * s,
                    width: self.width - 32. * s,
                    height: s,
                },
                0.,
                divider,
                1.,
            );
            y += 5.;
        }
        // The program that asked matters most; the policy's owner is the fallback.
        let context = if request.policy {
            self.view
                .custom
                .get("subject-name")
                .map(|name| RequestContext::RequestedBy(name))
                .or_else(|| {
                    self.view
                        .custom
                        .get("policy-publisher")
                        .map(|publisher| RequestContext::PolicySource(publisher))
                })
                .or_else(|| {
                    self.view
                        .custom
                        .get("subject-process")
                        .map(|process| RequestContext::RequestedFor(process))
                })
        } else {
            None
        };
        let context_value = match context {
            Some(
                RequestContext::RequestedBy(value)
                | RequestContext::PolicySource(value)
                | RequestContext::RequestedFor(value),
            ) => Some(value),
            None => None,
        };
        // Stacked sections, like the key card: who authenticates, then who asked.
        let photo = self.view.user_avatar.is_some() || self.view.avatar.is_some();
        let inset = if photo {
            let tile = Rect {
                x: self.left + 16. * s,
                y: self.top + (y + 14.) * s,
                width: 36. * s,
                height: 36. * s,
            };
            self.avatar(tile, 18. * s);
            64.
        } else {
            16.
        };
        let caption = self.caption();
        let width = self.width - (inset + 16.) * s;
        self.label(
            &fl!("session-authenticating-as"),
            (inset, y + 27.),
            13.,
            width,
            caption,
            Weight::MEDIUM,
        );
        self.label(name, (inset, y + 48.), 15.5, width, text, Weight::SEMIBOLD);
        if let (Some(context), Some(value)) = (context, context_value) {
            let y = y + ACCOUNT_SECTION;
            self.fill(
                Rect {
                    x: self.left + 16. * s,
                    y: self.top + y * s,
                    width: self.width - 32. * s,
                    height: s,
                },
                0.,
                divider,
                1.,
            );
            let width = self.width - 32. * s;
            self.label(
                &context.title(),
                (16., y + 27.),
                13.,
                width,
                caption,
                Weight::MEDIUM,
            );
            self.label(value, (16., y + 48.), 15.5, width, text, Weight::SEMIBOLD);
        }
    }

    fn request_code(&mut self, value: &str, x: f32, baseline: f32, width: f32) {
        let text = value.trim();
        let s = self.scale;
        self.styled(
            text,
            (self.left + x * s, self.top + baseline * s),
            14.5 * s,
            width,
            mix(self.palette.text, self.palette.muted, 0.15),
            None,
            Style::Code,
        );
    }

    /// The key's owner and details above its full-width fingerprint.
    fn key_identity(&mut self, subject: Option<&str>, fingerprint: &[String], divider: Rgb) {
        let s = self.scale;
        let text = self.palette.text;
        let caption = self.caption();
        let width = self.width - 32. * s;
        let mut base = 0.;
        if let Some(subject) = subject {
            let (name, details) = key_subject(subject);
            self.label(
                &fl!("session-key-identity"),
                (16., 27.),
                13.,
                width,
                caption,
                Weight::MEDIUM,
            );
            self.label(name, (16., 48.), 15.5, width, text, Weight::SEMIBOLD);
            if !details.is_empty() {
                let detail = mix(self.palette.muted, text, 0.3);
                self.label(
                    &details.join(" · "),
                    (16., 68.),
                    14.,
                    width,
                    detail,
                    Weight::NORMAL,
                );
            }
            base = KEY_IDENTITY;
            self.fill(
                Rect {
                    x: self.left + 16. * s,
                    y: self.top + base * s,
                    width,
                    height: s,
                },
                0.,
                divider,
                1.,
            );
        }
        self.label(
            &fl!("session-fingerprint"),
            (16., base + 28.),
            13.,
            width,
            caption,
            Weight::MEDIUM,
        );
        for (index, line) in fingerprint.iter().enumerate() {
            self.request_code(line.trim(), 16., base + 51. + index as f32 * 22., width);
        }
    }

    /// A stable colour per person for avatars without a picture, and the ink
    /// for their initials. Hues are muted to sit beside the theme accent.
    fn person_tone(&self, name: &str) -> (Rgb, Rgb) {
        const HUES: [Rgb; 7] = [
            Rgb(0x3d, 0x7a, 0x68),
            Rgb(0x3e, 0x66, 0x8c),
            Rgb(0x6a, 0x58, 0x8e),
            Rgb(0x8c, 0x52, 0x64),
            Rgb(0x8a, 0x6c, 0x3a),
            Rgb(0x4a, 0x7a, 0x4e),
            Rgb(0x56, 0x68, 0x7a),
        ];
        let hash = name.bytes().fold(0x811c_9dc5u32, |hash, byte| {
            (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
        });
        let hue = mix(
            HUES[hash as usize % HUES.len()],
            self.appearance.role(ColorRole::AccentSurface),
            0.12,
        );
        let fill = if self.light() {
            mix(hue, self.palette.surface, 0.72)
        } else {
            mix(hue, self.palette.background, 0.08)
        };
        let ink = if self.palette.text.contrast(fill) >= 4.5 {
            self.palette.text
        } else if self.light() {
            Rgb(0x10, 0x10, 0x10)
        } else {
            Rgb(0xff, 0xff, 0xff)
        };
        (fill, ink)
    }

    /// Secondary lines: subtitles and status that must stay easy to read.
    fn secondary(&self) -> Rgb {
        mix(self.palette.muted, self.palette.text, 0.4)
    }

    /// Small labels inside request cards.
    fn caption(&self) -> Rgb {
        self.secondary()
    }

    fn initial_tile(&mut self, name: &str, x: f32, center: f32, size: f32, selected: bool) {
        let s = self.scale;
        let tile = Rect {
            x: self.left + x * s,
            y: self.top + (center - size / 2.) * s,
            width: size * s,
            height: size * s,
        };
        let _ = selected;
        let (fill, ink) = self.person_tone(name);
        let initials = Self::initials(name);
        self.fill(tile, size / 2. * s, fill, 1.);
        if initials.is_empty() {
            self.icon(
                (tile.x + (size / 2. - 9.) * s, tile.y + (size / 2. - 9.) * s),
                0.75 * s,
                Emblem::Person,
                ink,
            );
        } else {
            self.styled(
                &initials,
                (tile.x + size * s / 2., self.top + (center + 4.5) * s),
                12.5 * s,
                (size - 6.) * s,
                ink,
                None,
                Style::Centered(Weight::SEMIBOLD),
            );
        }
    }

    /// One dialog footer: an optional disclosure link at the start, then the
    /// actions at the end with the primary action last.
    fn footer(
        &mut self,
        controls: &[&Control],
        disclosure: Option<&Control>,
        hits: &mut Vec<(usize, Rect)>,
    ) {
        let s = self.scale;
        let gap = 10. * s;
        let size = 14.5;
        let link_width = disclosure.map_or(0., |control| {
            self.view.text_width(&control.label, size * s).min(170. * s) + 40. * s
        });
        let widths: Vec<f32> = controls
            .iter()
            .map(|control| (self.view.text_width(&control.label, size * s) + 40. * s).max(100. * s))
            .collect();
        let total = widths.iter().sum::<f32>() + gap * controls.len().saturating_sub(1) as f32;
        let available = self.width
            - if disclosure.is_some() {
                link_width + 16. * s
            } else {
                0.
            };
        if total <= available {
            let mut x = self.left + self.width - total;
            for (control, width) in controls.iter().zip(widths) {
                let rect = Rect {
                    x,
                    y: self.top,
                    width,
                    height: BUTTON * s,
                };
                self.button(control, rect, size, hits);
                x += width + gap;
            }
        } else {
            // Equal widths over the full row, in rows of two above the link.
            let rows: Vec<_> = controls.chunks(2).collect();
            let count = rows.len() as f32;
            for (index, row) in rows.iter().enumerate() {
                let width = (self.width - gap * (row.len() - 1) as f32) / row.len() as f32;
                let y = self.top
                    - (count - 1. - index as f32) * (BUTTON + 10.) * s
                    - if disclosure.is_some() {
                        (BUTTON + 6.) * s
                    } else {
                        0.
                    };
                for (column, control) in row.iter().enumerate() {
                    let rect = Rect {
                        x: self.left + (width + gap) * column as f32,
                        y,
                        width,
                        height: BUTTON * s,
                    };
                    self.button(control, rect, size, hits);
                }
            }
        }
        if let Some(control) = disclosure {
            let rect = Rect {
                x: self.left - 10. * s,
                y: self.top,
                width: link_width,
                height: BUTTON * s,
            };
            self.link(control, rect, size, hits);
        }
    }

    fn button(&mut self, control: &Control, rect: Rect, size: f32, hits: &mut Vec<(usize, Rect)>) {
        let s = self.scale;
        let primary = self.view.primary_control == Some(control.id) && !self.view.busy;
        let light = self.light();
        let (fill, ink) = if primary {
            (
                self.palette.accent,
                self.appearance.role(ColorRole::OnAccent),
            )
        } else if light {
            (
                mix(self.palette.surface, self.palette.raised, 0.7),
                self.palette.text,
            )
        } else {
            (
                mix(self.palette.surface, self.palette.raised, 0.8),
                self.palette.text,
            )
        };
        let radius = BUTTON / 2. * s;
        // The default action is implied; a ring marks keyboard focus elsewhere.
        if control.selected && !primary {
            let focus = self.focus();
            self.frame(rect, radius, 1.75 * s, fill, focus);
        } else if primary {
            self.fill(rect, radius, fill, 1.);
        } else {
            let edge = mix(fill, self.palette.border, if light { 0.45 } else { 0.4 });
            self.frame(rect, radius, s, fill, edge);
        }
        let label = Self::clean(&control.label);
        self.styled(
            &label,
            (
                rect.x + rect.width / 2.,
                rect.y + rect.height / 2. + size * 0.36 * s,
            ),
            size * s,
            rect.width - 20. * s,
            ink,
            None,
            Style::Centered(if primary {
                Weight::SEMIBOLD
            } else {
                Weight::MEDIUM
            }),
        );
        self.hit(hits, control.id, rect);
    }

    /// A secondary text action with a trailing chevron; the accent stays with
    /// the primary action.
    fn link(&mut self, control: &Control, rect: Rect, size: f32, hits: &mut Vec<(usize, Rect)>) {
        let s = self.scale;
        let focus = self.focus();
        let ink = mix(self.palette.muted, self.palette.text, 0.55);
        if control.selected {
            let surface = self.appearance.role(ColorRole::AccentSurface);
            self.frame(rect, 10. * s, 1.5 * s, surface, focus);
        }
        let label = Self::clean(&control.label);
        let width = self
            .view
            .text_width(&label, size * s)
            .min(rect.width - 40. * s);
        self.styled(
            &label,
            (
                rect.x + 10. * s,
                rect.y + rect.height / 2. + size * 0.36 * s,
            ),
            size * s,
            rect.width - 40. * s,
            ink,
            None,
            Style::Leading(Weight::MEDIUM),
        );
        self.icon(
            (rect.x + 14. * s + width, rect.y + rect.height / 2. - 8. * s),
            s * 2. / 3.,
            Emblem::Chevron,
            ink,
        );
        self.hit(hits, control.id, rect);
    }

    fn buttons(
        &mut self,
        controls: &[&Control],
        treatment: Treatment,
        panel: &AuthenticationPanel,
        hits: &mut Vec<(usize, Rect)>,
    ) {
        let s = self.scale;
        let gap = 10. * s;
        let width = (self.width - gap * (controls.len() - 1) as f32) / controls.len() as f32;
        for (index, control) in controls.iter().enumerate() {
            let selected = control.selected;
            let back_label;
            let label = if control.kind == ControlKind::Back {
                back_label = fl!(
                    "session-back-label",
                    arrow = if self.rtl { "→" } else { "←" },
                    label = control.label.as_str()
                );
                back_label.as_str()
            } else {
                control.label.as_str()
            };
            let rect = Rect {
                x: self.left + (width + gap) * index as f32,
                y: self.top,
                width,
                height: panel.control_height(treatment) * s,
            };
            if control.kind == ControlKind::Back {
                self.back_button(control, label, rect, controls.len() == 1, hits);
                continue;
            }
            if treatment == Treatment::Action {
                let mut control = (*control).clone();
                control.label = label.to_owned();
                self.button(
                    &control,
                    Rect {
                        height: 44. * s,
                        ..rect
                    },
                    15.,
                    hits,
                );
                continue;
            }
            if treatment == Treatment::Utility {
                let link = Rect {
                    x: rect.x - 10. * s,
                    width: rect.width + 10. * s,
                    ..rect
                };
                let mut control = (*control).clone();
                control.label = label.to_owned();
                self.link(&control, link, 14., hits);
                continue;
            }
            // Choices: list rows in choosers, buttons with a trailing icon elsewhere.
            let current = panel.list_actions && panel.checked_control == Some(control.id);
            let fill = if selected {
                self.appearance.role(ColorRole::AccentSurface)
            } else if self.light() {
                mix(self.palette.surface, self.palette.raised, 0.6)
            } else {
                mix(self.palette.surface, self.palette.raised, 0.7)
            };
            let edge = if selected {
                self.focus()
            } else {
                mix(fill, self.palette.border, 0.4)
            };
            self.frame(
                rect,
                12. * s,
                if selected { 1.5 } else { 1. } * s,
                fill,
                edge,
            );
            let ink = self.palette.text;
            if panel.list_actions {
                let subtitle = panel.choice_details.get(&control.id);
                let center = rect.height / s / 2.;
                let offset = (rect.x - self.left) / s;
                if matches!(panel.emblem, Emblem::Accounts) {
                    self.initial_tile(label, offset + 12., center, 34., current);
                } else {
                    let tile = Rect {
                        x: rect.x + 12. * s,
                        y: rect.y + (center - 17.) * s,
                        width: 34. * s,
                        height: 34. * s,
                    };
                    let tint = self.appearance.role(ColorRole::AccentSurface);
                    let tint = if selected {
                        mix(tint, self.palette.surface, 0.5)
                    } else {
                        tint
                    };
                    self.fill(tile, 10. * s, tint, 1.);
                    let focus = self.focus();
                    self.icon(
                        (tile.x + 8. * s, tile.y + 8. * s),
                        0.75 * s,
                        Emblem::Display,
                        focus,
                    );
                }
                let available = width - if current { 150. * s } else { 80. * s };
                self.label(
                    label,
                    (
                        offset + 58.,
                        center + if subtitle.is_some() { -3. } else { 5. },
                    ),
                    15.,
                    available,
                    ink,
                    Weight::SEMIBOLD,
                );
                if let Some(detail) = subtitle {
                    let muted = self.palette.muted;
                    self.label(
                        detail,
                        (offset + 58., center + 16.),
                        13.5,
                        available,
                        muted,
                        Weight::NORMAL,
                    );
                }
                if current {
                    let focus = self.focus();
                    let chip = Rect {
                        x: rect.x + rect.width - 100. * s,
                        y: rect.y + (center - 13.) * s,
                        width: 86. * s,
                        height: 26. * s,
                    };
                    let chip_fill = if selected {
                        mix(fill, self.palette.surface, 0.6)
                    } else {
                        self.appearance.role(ColorRole::AccentSurface)
                    };
                    self.fill(chip, 13. * s, chip_fill, 1.);
                    self.icon(
                        (chip.x + 8. * s, chip.y + 4. * s),
                        0.75 * s,
                        Emblem::Check,
                        focus,
                    );
                    self.label(
                        &fl!("session-current"),
                        ((chip.x - self.left) / s + 30., center + 4.5),
                        13.,
                        52. * s,
                        focus,
                        Weight::SEMIBOLD,
                    );
                }
                self.hit(hits, control.id, rect);
                continue;
            }
            let identity = control.kind == ControlKind::Identity
                && self.avatar(
                    Rect {
                        x: rect.x + 12. * s,
                        y: rect.y + (rect.height - 26. * s) / 2.,
                        width: 26. * s,
                        height: 26. * s,
                    },
                    13. * s,
                );
            let x = rect.x + if identity { 48. } else { 14. } * s;
            self.styled(
                label,
                (x, rect.y + rect.height / 2. + 14. * 0.36 * s),
                14. * s,
                rect.width - if identity { 90. } else { 56. } * s,
                ink,
                None,
                Style::Leading(Weight::MEDIUM),
            );
            let icon = panel
                .choices
                .iter()
                .find(|(id, _)| *id == control.id)
                .map_or(Emblem::Cycle, |(_, icon)| *icon);
            let muted = self.palette.muted;
            self.icon(
                (
                    rect.x + rect.width - 34. * s,
                    rect.y + rect.height / 2. - 8. * s,
                ),
                s * 2. / 3.,
                icon,
                muted,
            );
            self.hit(hits, control.id, rect);
        }
    }

    fn back_button(
        &mut self,
        control: &Control,
        label: &str,
        rect: Rect,
        alone: bool,
        hits: &mut Vec<(usize, Rect)>,
    ) {
        let s = self.scale;
        let width = (self.view.text_width(label, 14. * s) + 32. * s).min(rect.width);
        let rect = Rect {
            width,
            height: 36. * s,
            ..rect
        };
        let fill = mix(self.palette.surface, self.palette.raised, 0.7);
        let edge = if control.selected {
            self.focus()
        } else {
            mix(fill, self.palette.border, 0.45)
        };
        let border = if control.selected { 1.5 } else { 1. } * s;
        self.frame(rect, 10. * s, border, fill, edge);
        let text = self.palette.text;
        self.styled(
            label,
            (rect.x + 16. * s, rect.y + 23. * s),
            14. * s,
            width,
            text,
            None,
            Style::Leading(Weight::MEDIUM),
        );
        self.hit(hits, control.id, rect);
        if alone {
            self.chooser_shortcuts();
        }
    }

    fn chooser_shortcuts(&mut self) {
        let s = self.scale;
        let label = fl!("session-chooser-shortcuts");
        let width = self.view.text_width(&label, 13.5 * s);
        let muted = self.palette.muted;
        self.label(
            &label,
            ((self.width - width) / s, 23.),
            13.5,
            width + 2. * s,
            muted,
            Weight::NORMAL,
        );
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

/// A key's name and up to two detail lines from "Name · detail · detail".
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
