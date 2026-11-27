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
use super::iced::{Content as ButtonContent, Leading, Scene};
use super::raster::Emblem;
use super::style::{self, Kind, Look};
use super::{Control, ControlKind, SessionView, wrap};
use crate::layout::Rect;
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
    /// Labelled facts shown in place of the request card (request details).
    pub facts: Vec<(String, String)>,
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
const FORM_WIDTH: f32 = 420.;
const AVATAR: f32 = 96.;
const FIELD: f32 = 52.;
const SHEET_WIDTH: f32 = 420.;
const SHEET_PADDING: f32 = 24.;
const DIALOG_PADDING: f32 = 16.;
const BUTTON: f32 = 42.;
/// Space between a dialog's content and its footer actions, which sit on a band.
const FOOTER_GAP: f32 = 42.;
/// Below the footer actions, inside the window margin; with it, the band's
/// bottom padding matches its top.
const FOOTER_BOTTOM: f32 = 4.;
const CHIP: f32 = 40.;
/// A power action drawn as an icon above its label.
const TILE: f32 = 104.;
/// Room under a full-screen form's field for two status lines.
/// One status line fits without moving anything below; a second (an error
/// with the Caps Lock warning) moves only the account action.
const STATUS_RESERVE: f32 = 33.;
/// Space between the clock and a full-screen card.
const SHEET_OFFSET: f32 = 14.;

enum Content<'a> {
    /// Form: avatar, name and subtitle, centred.
    Identity(Vec<&'a Control>),
    /// Sheet and dialog: emblem tile, title and subtitle.
    Heading,
    Copy(Vec<String>, Tone),
    Request(Request<'a>),
    /// Captioned values in one card.
    Facts(Vec<(&'a str, Vec<String>)>),
    /// A strength meter from -1 (rejected) to 1.
    Meter(f32),
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

/// A fact's caption and padding, before its value lines.
const FACT: f32 = 38.;

/// One caption-and-value section of a request card.
const ACCOUNT_SECTION: f32 = 68.;

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
    /// A form drawn as a card (the start-up account list).
    card: bool,
    /// The instruction while the system checks a response.
    checking: String,
    checking_instruction: String,
}

impl AuthenticationPanel {
    fn control_height(&self, treatment: Treatment) -> f32 {
        match treatment {
            Treatment::Utility => 36.,
            Treatment::Choice if self.list_actions => 68.,
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
                Mode::Sheet => SHEET_OFFSET + SHEET_PADDING,
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
            card: false,
            checking: fl!("session-checking"),
            checking_instruction: fl!("session-checking-password"),
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
            let gap = if view.message.is_empty() { gap } else { 4. };
            self.copy(vec![fl!("session-caps-lock-on")], gap, Tone::Caps);
        }
    }

    fn form(
        &mut self,
        view: &'a SessionView,
        mut actions: Vec<&'a Control>,
        choices: Vec<&'a Control>,
        utilities: Vec<&'a Control>,
        mut links: Vec<&'a Control>,
        accounts: Vec<&'a Control>,
    ) {
        if !accounts.is_empty() {
            // The start-up list is the same card as the account switcher;
            // another account is one more row of it.
            self.card = true;
            self.width = SHEET_WIDTH;
            self.height = SHEET_OFFSET + SHEET_PADDING;
            self.push(Content::Heading, 0., 46.);
            let mut accounts = accounts;
            accounts.append(&mut links);
            for (index, control) in accounts.into_iter().enumerate() {
                self.push(
                    Content::Buttons(vec![control], Treatment::Choice),
                    if index == 0 { 20. } else { 8. },
                    68.,
                );
            }
            self.status(view, 46, 14.);
            self.buttons(&utilities, Treatment::Utility, 8., 36.);
            self.height += SHEET_PADDING;
            return;
        }
        let (_, subtitle) = self.titles(view);
        // Space the identity apart from a clock and date stacked above it.
        // Switching account is a pill just below the field (and its status),
        // so the identity above the field stays name and instruction only.
        let switch: Vec<&Control> = Vec::new();
        // The instruction line is always reserved, so the field never moves
        // when it changes or hides (while a response is checked).
        let _ = subtitle;
        let height = AVATAR + 16. + 30. + 28. + if switch.is_empty() { 0. } else { 36. };
        self.push(Content::Identity(switch), 30., height);
        let primary = actions
            .iter()
            .position(|control| view.primary_control == Some(control.id))
            .map(|index| actions.remove(index));
        if view.prompt.is_none() && view.busy && !view.hide_input {
            // Waiting on the system: the field stays in place, quiet, with a
            // spinner; the unavailable submit action is not repeated below it.
            actions.retain(|control| control.enabled);
            self.push(
                Content::Field {
                    label: Vec::new(),
                    placeholder: view.field_placeholder(),
                    submit: None,
                },
                26.,
                FIELD,
            );
        } else if view.prompt.is_some() && !self.panel.confirm_identity {
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
        // Messages appear in a reserved slot, so nothing above or below moves.
        let field_end = self.height;
        self.status(view, 46, 12.);
        self.height = self.height.max(field_end + STATUS_RESERVE);
        if !links.is_empty() {
            self.push(Content::Links(links), 16., 34.);
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
            let height = self.row_height(&row, treatment);
            self.push(
                Content::Buttons(row, treatment),
                if index == 0 { 20. } else { 8. },
                height,
            );
        }
        if !choices.is_empty() {
            self.push(Content::Divider, 18., 1.);
            self.buttons(&choices, Treatment::Choice, 12., 44.);
        }
        if !utilities.is_empty() {
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
            .flat_map(|text| wrap(text, 54, 8))
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
        if !panel.facts.is_empty() {
            let facts: Vec<_> = panel
                .facts
                .iter()
                .map(|(caption, value)| {
                    // Long values scroll with the card rather than being cut.
                    (caption.as_str(), wrap(value, 60, 64))
                })
                .collect();
            let height = facts
                .iter()
                .map(|(_, lines)| FACT + 20. * lines.len() as f32)
                .sum();
            self.push(Content::Facts(facts), 22., height);
            self.footer_disclosure = disclosure;
            let (primary, mut secondary): (Vec<_>, Vec<_>) = actions
                .into_iter()
                .partition(|control| view.primary_control == Some(control.id));
            secondary.extend(primary);
            self.footer = secondary;
            let _ = (choices, utilities);
            return;
        }
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
        if let Some(level) = view
            .custom
            .get("strength")
            .and_then(|value| value.parse::<f32>().ok())
        {
            self.push(Content::Meter(level), 12., 6.);
        }
        let field_end = self.height;
        self.status(view, 64, 6.);
        if view.prompt.is_some() && !view.hide_input {
            // A consumer's helper fills the reserved line until something is reported.
            if self.height == field_end && !view.busy {
                if let Some(hint) = view.custom.get("field-hint").filter(|s| !s.is_empty()) {
                    self.copy(vec![hint.clone()], 6., Tone::Muted);
                }
            }
            // One status line is always reserved, so a failure never moves the
            // actions; until something is reported it says how to continue.
            self.height = self.height.max(field_end + 28.);
        }
        let (primary, mut secondary): (Vec<_>, Vec<_>) = actions
            .into_iter()
            .partition(|control| view.primary_control == Some(control.id));
        secondary.extend(primary);
        self.footer = secondary;
        self.footer_disclosure = disclosure;
    }

    /// The identity title and subtitle shown by the form and the sheet heading.
    fn titles<'s>(&'s self, view: &'s SessionView) -> (&'s str, &'s str) {
        let panel = self.panel;
        // While a submitted response is checked, the instruction says so.
        if self.mode == Mode::Form && view.busy && view.prompt.is_none() && view.message.is_empty() {
            let title = if self.username {
                view.username.as_str()
            } else {
                view.custom
                    .get("request-title")
                    .map_or(panel.heading.as_str(), String::as_str)
            };
            return (title, self.checking_instruction.as_str());
        }
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
                (height - FOOTER_BOTTOM - BUTTON).max(self.height + FOOTER_GAP - DIALOG_PADDING);
            height.max(footer_top + BUTTON + FOOTER_BOTTOM)
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
            FOOTER_GAP + BUTTON + FOOTER_BOTTOM - DIALOG_PADDING
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
        let look = Look::new(settings);
        let rtl = canopa_i18n::direction().is_rtl();
        let padding = match self.mode {
            Mode::Form if self.card => SHEET_PADDING,
            Mode::Form => 0.,
            Mode::Sheet => SHEET_PADDING,
            Mode::Dialog => DIALOG_PADDING,
        };
        if self.mode == Mode::Sheet || self.card {
            scene.raised_surface(
                Rect {
                    y: bounds.y + SHEET_OFFSET * scale,
                    height: (self.height - SHEET_OFFSET) * scale,
                    ..bounds
                },
                28. * scale,
                look.glass_card(),
                look.glass_edge(),
                look.shadow(40. * scale),
            );
        }
        let footer_top = (self.height_reserve.max(self.height) - FOOTER_BOTTOM - BUTTON)
            .max(self.height + FOOTER_GAP - DIALOG_PADDING);
        let has_footer = self.mode == Mode::Dialog
            && (!self.footer.is_empty() || self.footer_disclosure.is_some());
        if has_footer && corners {
            // The actions sit on a full-width band along the window's bottom edge.
            let top = bounds.y + (footer_top - 20.) * scale;
            let band = Rect {
                x: 0.,
                y: top,
                width: screen.width,
                height: (screen.height - top).max(1.),
            };
            scene.surface(
                band,
                0.,
                look.footer(),
                iced::Color::TRANSPARENT,
                0.,
                iced::Shadow::default(),
            );
            scene.surface(
                Rect {
                    height: scale.max(1.),
                    ..band
                },
                0.,
                look.hairline(),
                iced::Color::TRANSPARENT,
                0.,
                iced::Shadow::default(),
            );
        }
        let left = bounds.x + padding * scale;
        let width = bounds.width - 2. * padding * scale;
        let (title, subtitle) = self.titles(view);
        let spare = if self.mode == Mode::Dialog {
            ((footer_top - (self.height - DIALOG_PADDING) - FOOTER_GAP) / 3.).clamp(0., 14.)
        } else {
            0.
        };
        for (index, (offset, content)) in self.rows.iter().enumerate() {
            let top = bounds.y + (offset + if index > 0 { spare } else { 0. }) * scale;
            let shaken = matches!(
                content,
                Content::Field { .. } | Content::Copy(_, Tone::Notice | Tone::Caps)
            ) && view.failed;
            let resting = scene.offset;
            if shaken {
                scene.offset.0 += scene.shake;
            }
            let row = Rect {
                x: left,
                y: top,
                width,
                height: 44. * scale,
            };
            self.populate_row_content(view, settings, scene, content, row, top, left, width, scale, &look, rtl, title, subtitle);
            scene.offset = resting;
        }
        if has_footer {
            self.populate_footer(
                view,
                settings,
                scene,
                Rect {
                    x: left,
                    y: bounds.y + footer_top * scale,
                    width,
                    height: BUTTON * scale,
                },
                scale,
            );
        }
        if corners {
            self.populate_corners(view, settings, scene, scale, screen);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn populate_row_content(
        &self,
        view: &SessionView,
        settings: &crate::settings::Settings,
        scene: &mut Scene,
        content: &Content<'_>,
        row: Rect,
        top: f32,
        left: f32,
        width: f32,
        scale: f32,
        look: &Look,
        rtl: bool,
        title: &str,
        subtitle: &str,
    ) {
        let palette = look.palette;
        {
            match content {
                Content::Identity(switch) => {
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
                        scene.anonymous(avatar, settings);
                    }
                    scene.label(
                        title,
                        Rect {
                            y: top + (AVATAR + 16.) * scale,
                            height: 32. * scale,
                            ..row
                        },
                        25. * scale,
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
                        16. * scale,
                        palette.muted,
                        true,
                        Weight::Normal,
                    );
                    if !switch.is_empty() {
                        let y = top
                            + (AVATAR + 50. + if subtitle.is_empty() { 0. } else { 28. }) * scale;
                        self.populate_links(view, settings, scene, switch, left, width, y, scale);
                    }
                }
                Content::Heading => {
                    let sheet = self.mode == Mode::Sheet || self.card;
                    // The start-up account list is titled as a choice, not as an account.
                    let choose = fl!("session-choose-account");
                    let (title, subtitle, emblem) = if self.card {
                        (choose.as_str(), "", Emblem::Accounts)
                    } else {
                        (title, subtitle, self.panel.emblem)
                    };
                    let tile_side = 44. * scale;
                    let tile = Rect {
                        x: if rtl { left + width - tile_side } else { left },
                        y: top,
                        width: tile_side,
                        height: tile_side,
                    };
                    // A warning heading (a destructive confirmation) uses the danger tone.
                    let tone = if emblem == Emblem::Alert {
                        palette.danger
                    } else {
                        palette.accent
                    };
                    scene.surface(
                        tile,
                        if sheet { tile_side / 2. } else { 12. * scale },
                        style::rgba(tone, look.tint(1.).a),
                        iced::Color::TRANSPARENT,
                        0.,
                        iced::Shadow::default(),
                    );
                    let inner = 24. * scale;
                    scene.icon(
                        emblem,
                        Rect {
                            x: tile.x + (tile_side - inner) / 2.,
                            y: tile.y + (tile_side - inner) / 2.,
                            width: inner,
                            height: inner,
                        },
                        tone,
                    );
                    let text_x = left + if rtl { 0. } else { 58. * scale };
                    let text_width = width - 58. * scale;
                    let single = subtitle.is_empty();
                    scene.label(
                        title,
                        Rect {
                            x: text_x,
                            y: if single { top + 9. * scale } else { top },
                            width: text_width,
                            height: 26. * scale,
                        },
                        if sheet { 19. } else { 18. } * scale,
                        palette.text,
                        false,
                        Weight::Semibold,
                    );
                    scene.label(
                        subtitle,
                        Rect {
                            x: text_x,
                            y: top + 25. * scale,
                            width: text_width,
                            height: 20. * scale,
                        },
                        13.5 * scale,
                        // A warning heading's consequence carries its tone.
                        if emblem == Emblem::Alert {
                            palette.danger
                        } else {
                            palette.muted
                        },
                        false,
                        if emblem == Emblem::Alert {
                            Weight::Medium
                        } else {
                            Weight::Normal
                        },
                    );
                }
                Content::Copy(lines, tone) => {
                    // Status lines over a light wallpaper need more weight than
                    // the palette's warning tone gives on its own.
                    let warning = if look.dark {
                        palette.warning
                    } else {
                        super::raster::mix(palette.warning, palette.text, 0.45)
                    };
                    let strength = view
                        .custom
                        .get("strength")
                        .and_then(|value| value.parse::<f32>().ok())
                        .filter(|_| *tone == Tone::Notice && !view.failed);
                    let color = match tone {
                        Tone::Notice if view.failed => palette.danger,
                        Tone::Notice if strength.is_some() => {
                            strength_color(strength.unwrap_or(0.), &palette, warning)
                        }
                        Tone::Caps => warning,
                        Tone::Body => palette.text,
                        _ => palette.muted,
                    };
                    let size = if self.mode == Mode::Form { 15. } else { 14. } * scale;
                    let line = if self.mode == Mode::Form { 21. } else { 22. } * scale;
                    let icon = match tone {
                        Tone::Notice if view.failed => Some(Emblem::Alert),
                        Tone::Caps => Some(Emblem::CapsLock),
                        _ => None,
                    };
                    if let (Some(icon), [only]) = (icon, lines.as_slice()) {
                        let side = 17. * scale;
                        let gap = 7. * scale;
                        let text_width = (view.text_width(only, size) + 4. * scale)
                            .min(width - side - gap);
                        let total = side + gap + text_width;
                        let start = if self.mode == Mode::Form {
                            left + (width - total) / 2.
                        } else if rtl {
                            left + width - total
                        } else {
                            left
                        };
                        let (icon_x, text_x) = if rtl {
                            (start + text_width + gap, start)
                        } else {
                            (start, start + side + gap)
                        };
                        scene.icon(
                            icon,
                            Rect {
                                x: icon_x,
                                y: top + (line - side) / 2.,
                                width: side,
                                height: side,
                            },
                            color,
                        );
                        scene.label(
                            only.clone(),
                            Rect {
                                x: text_x,
                                y: top,
                                width: text_width,
                                height: line,
                            },
                            size,
                            color,
                            false,
                            Weight::Medium,
                        );
                        return;
                    }
                    // A dialog's body text starts at its title's column.
                    let indent = if *tone == Tone::Body && self.mode == Mode::Dialog {
                        58. * scale
                    } else {
                        0.
                    };
                    scene.lines(
                        lines,
                        Rect {
                            x: row.x + if rtl { 0. } else { indent },
                            width: row.width - indent,
                            height: line * lines.len() as f32,
                            ..row
                        },
                        size,
                        line,
                        color,
                        self.mode == Mode::Form,
                        Weight::Normal,
                    );
                }
                Content::Request(request) => {
                    self.populate_request(view, settings, scene, request, row, scale);
                }
                Content::Facts(facts) => {
                    let height = facts
                        .iter()
                        .map(|(_, lines)| FACT + 20. * lines.len() as f32)
                        .sum::<f32>();
                    scene.surface(
                        Rect {
                            height: height * scale,
                            ..row
                        },
                        14. * scale,
                        style::rgba(palette.raised, if look.dark { 0.75 } else { 0.7 }),
                        look.hairline(),
                        scale,
                        iced::Shadow::default(),
                    );
                    let mut y = top;
                    for (index, (caption, lines)) in facts.iter().enumerate() {
                        let area = Rect {
                            x: left + 16. * scale,
                            width: width - 32. * scale,
                            y,
                            height: 18. * scale,
                        };
                        if index > 0 {
                            scene.surface(
                                Rect {
                                    height: scale.max(1.),
                                    ..area
                                },
                                0.,
                                look.hairline(),
                                iced::Color::TRANSPARENT,
                                0.,
                                iced::Shadow::default(),
                            );
                        }
                        scene.label(
                            *caption,
                            Rect {
                                y: y + 10. * scale,
                                ..area
                            },
                            13. * scale,
                            palette.muted,
                            false,
                            Weight::Medium,
                        );
                        // Identifiers (an action ID) read as code.
                        let identifier = lines.len() == 1
                            && !lines[0].contains(' ')
                            && lines[0].matches('.').count() >= 2;
                        if identifier {
                            scene.code(
                                &lines[0],
                                Rect {
                                    y: y + 29. * scale,
                                    height: 20. * scale,
                                    ..area
                                },
                                13. * scale,
                                palette.text,
                            );
                        } else {
                            // The first line is the value; further lines (a process
                            // ID) are secondary.
                            for (index, line) in lines.iter().enumerate() {
                                scene.label(
                                    line.clone(),
                                    Rect {
                                        y: y + (29. + 20. * index as f32) * scale,
                                        height: 20. * scale,
                                        ..area
                                    },
                                    if index == 0 { 14.5 } else { 13. } * scale,
                                    if index == 0 { palette.text } else { palette.muted },
                                    false,
                                    if index == 0 { Weight::Medium } else { Weight::Normal },
                                );
                            }
                        }
                        y += (FACT + 20. * lines.len() as f32) * scale;
                    }
                }
                Content::Meter(level) => {
                    let segments = 4;
                    let gap = 6. * scale;
                    let each = (width - gap * (segments - 1) as f32) / segments as f32;
                    // The same bands as the agent's rating labels.
                    let filled = match *level {
                        level if level < 0. => 1,
                        level if level <= 0. => 0,
                        level if level < 0.30 => 1,
                        level if level < 0.60 => 2,
                        level if level < 0.85 => 3,
                        _ => 4,
                    };
                    let color = strength_color(*level, &palette, palette.warning);
                    for index in 0..segments {
                        let column = if rtl { segments - 1 - index } else { index };
                        scene.surface(
                            Rect {
                                x: left + (each + gap) * column as f32,
                                y: top,
                                width: each,
                                height: 6. * scale,
                            },
                            3. * scale,
                            if index < filled {
                                style::rgba(color, 1.)
                            } else {
                                look.hairline()
                            },
                            iced::Color::TRANSPARENT,
                            0.,
                            iced::Shadow::default(),
                        );
                    }
                }
                Content::Divider => scene.surface(
                    Rect {
                        height: scale.max(1.),
                        ..row
                    },
                    0.,
                    look.hairline(),
                    iced::Color::TRANSPARENT,
                    0.,
                    iced::Shadow::default(),
                ),
                Content::Field {
                    label,
                    placeholder,
                    submit,
                } => {
                    let form = self.mode == Mode::Form;
                    let label_height = if label.is_empty() {
                        0.
                    } else {
                        21. * label.len() as f32 + 8.
                    };
                    if !label.is_empty() {
                        scene.lines(
                            label,
                            Rect {
                                height: label_height * scale - 6. * scale,
                                ..row
                            },
                            13. * scale,
                            21. * scale,
                            palette.muted,
                            false,
                            Weight::Medium,
                        );
                    }
                    let field = Rect {
                        y: top + label_height * scale,
                        height: if form { FIELD } else { BUTTON } * scale,
                        ..row
                    };
                    let radius = if form { FIELD / 2. } else { 10. } * scale;
                    let active = view.prompt.is_some() && !view.busy;
                    let ring = if view.failed && view.secret.is_empty() {
                        palette.danger
                    } else {
                        look.focus
                    };
                    if form {
                        if active {
                            // One soft halo around the pill, in the field's state colour.
                            scene.surface(
                                Rect {
                                    x: field.x - 3. * scale,
                                    y: field.y - 3. * scale,
                                    width: field.width + 6. * scale,
                                    height: field.height + 6. * scale,
                                },
                                radius + 3. * scale,
                                iced::Color::TRANSPARENT,
                                style::rgba(ring, 0.24),
                                3. * scale,
                                iced::Shadow::default(),
                            );
                        }
                        scene.raised_surface(
                            field,
                            radius,
                            look.glass(),
                            if active {
                                style::rgba(ring, 1.)
                            } else {
                                look.glass_edge()
                            },
                            iced::Shadow {
                                color: iced::Color::from_rgba(0., 0., 0., if look.dark { 0.22 } else { 0.07 }),
                                offset: iced::Vector::new(0., 4. * scale),
                                blur_radius: 14. * scale,
                            },
                        );
                    } else {
                        scene.surface(
                            field,
                            radius,
                            look.inset(),
                            if active {
                                style::rgba(ring, 1.)
                            } else {
                                look.hairline()
                            },
                            if active { 2. * scale } else { scale },
                            iced::Shadow::default(),
                        );
                    }
                    let inset = if form { 6. } else { 4. } * scale;
                    let button_side = field.height - 2. * inset;
                    if let (Some(control), true) = (submit, form) {
                        *view.last_submit.borrow_mut() = Some(control.label.clone());
                    }
                    // While checking, the full-screen field keeps its submit and
                    // reveal slots so nothing jumps.
                    let held = (form && submit.is_none() && view.busy)
                        .then(|| view.last_submit.borrow().clone())
                        .flatten();
                    let reveal_width = if view.can_reveal() || held.is_some() {
                        button_side.min(36. * scale) + 10. * scale
                    } else {
                        0.
                    };
                    let submit_width = match (submit, &held) {
                        (Some(control), _) if form => {
                            // Wide enough for "Checking…" too, so the button never resizes.
                            let checking = view.text_width(&self.checking, 14. * scale) + 26. * scale;
                            (view.text_width(&control.label, 14. * scale).max(checking)
                                + 50. * scale)
                                .max(button_side)
                                + inset
                        }
                        (Some(_), _) => button_side + inset,
                        (None, Some(label)) => {
                            let checking = view.text_width(&self.checking, 14. * scale) + 26. * scale;
                            (view.text_width(label, 14. * scale).max(checking) + 50. * scale)
                                .max(button_side)
                                + inset
                        }
                        (None, None) => 0.,
                    };
                    let caps_badge = false;
                    let caps_width = if caps_badge { 26. * scale } else { 0. };
                    let trailing = reveal_width + submit_width + caps_width;
                    // While the system checks, the response stays, dimmed and read-only.
                    let resting_alpha = scene.alpha;
                    if view.busy {
                        scene.alpha *= 0.5;
                    }
                    scene.field(
                        view,
                        settings,
                        Rect {
                            x: field.x
                                + 3. * scale
                                + if rtl { trailing } else { 0. }
                                + if form && !rtl { 6. * scale } else { 0. },
                            y: field.y + 3. * scale,
                            width: (field.width - trailing - 6. * scale - if form { 6. * scale } else { 0. })
                                .max(1.),
                            height: field.height - 6. * scale,
                        },
                        scale,
                        placeholder,
                    );
                    scene.alpha = resting_alpha;
                    // Trailing items, from the outer edge inward.
                    let mut edge = if rtl {
                        field.x + inset
                    } else {
                        field.x + field.width - inset
                    };
                    let mut place = |width: f32| {
                        if rtl {
                            let x = edge;
                            edge += width;
                            x
                        } else {
                            edge -= width;
                            edge
                        }
                    };
                    if let (None, true, true) = (submit, form, view.busy) {
                        // The submit button stays, unavailable, with the spinner in it.
                        let slot = if held.is_some() {
                            submit_width - inset
                        } else {
                            button_side
                        };
                        let x = place(slot);
                        let circle = Rect {
                            x,
                            y: field.y + inset,
                            width: slot,
                            height: button_side,
                        };
                        scene.surface(
                            circle,
                            button_side / 2.,
                            style::rgba(palette.accent, 1.),
                            iced::Color::TRANSPARENT,
                            0.,
                            iced::Shadow::default(),
                        );
                        // "Checking…" beside the spinner, centred as one group in the
                        // button, which keeps its width.
                        // The instruction above says "Checking your password…".
                        let label_width = 0.;
                        let spinner_side = (button_side * 0.56).min(22. * scale);
                        let content = spinner_side;
                        let start = x + (slot - content) / 2.;
                        if label_width > 0. {
                            scene.label(
                                self.checking.clone(),
                                Rect {
                                    x: start + spinner_side + 8. * scale,
                                    y: field.y + inset,
                                    width: label_width + 4. * scale,
                                    height: button_side,
                                },
                                14. * scale,
                                palette.text,
                                false,
                                Weight::Medium,
                            );
                        }
                        let x = start + spinner_side / 2. - button_side / 2.;
                        let angle = scene.spinner.unwrap_or(0.);
                        let side = (button_side * 0.56).min(22. * scale);
                        scene.spinner(
                            Rect {
                                x: x + (button_side - side) / 2.,
                                y: field.y + (field.height - side) / 2.,
                                width: side,
                                height: side,
                            },
                            angle,
                            // The button's label colour, at full strength.
                            if look.dark { look.on_accent } else { canopa_theme::Rgb(255, 255, 255) },
                            iced::Color::from_rgba(1., 1., 1., if look.dark { 0.2 } else { 0.3 }),
                        );
                        place(8. * scale);
                        if false {
                            // The reveal control's place, quiet while unavailable.
                            let side = 20. * scale;
                            let x = place(reveal_width - 10. * scale);
                            let resting = scene.alpha;
                            scene.alpha *= 0.35;
                            scene.icon(
                                Emblem::Eye,
                                Rect {
                                    x: x + (reveal_width - 10. * scale - side) / 2.,
                                    y: field.y + (field.height - side) / 2.,
                                    width: side,
                                    height: side,
                                },
                                palette.text,
                            );
                            scene.alpha = resting;
                        }
                    }
                    if let Some(control) = submit {
                        if form {
                            // The default action says what it does ("Unlock", "Sign in").
                            let width = submit_width - inset;
                            let x = place(width);
                            scene.styled_button(
                                settings,
                                control,
                                Rect {
                                    x,
                                    y: field.y + inset,
                                    width,
                                    height: button_side,
                                },
                                scale,
                                // Nothing typed yet: the action rests, faded, until there is.
                                if view.busy {
                                    Kind::Glass
                                } else if view.secret.is_empty() {
                                    Kind::Resting
                                } else {
                                    Kind::Primary
                                },
                                button_side / 2.,
                                ButtonContent::Chip {
                                    icon: None,
                                    label: &control.label,
                                    trailing: Some(Emblem::Arrow),
                                },
                            );
                            place(4. * scale);
                        } else {
                            let x = place(button_side);
                            scene.icon_button(
                                view,
                                settings,
                                control,
                                Emblem::Arrow,
                                Rect {
                                    x,
                                    y: field.y + inset,
                                    width: button_side,
                                    height: button_side,
                                },
                                scale,
                            );
                        }
                        place(4. * scale);
                    }
                    if view.can_reveal() {
                        let control = Control::new(
                            crate::input::REVEAL_CONTROL,
                            if view.revealed() {
                                fl!("session-hide-response")
                            } else {
                                fl!("session-show-response")
                            },
                        );
                        let side = button_side.min(36. * scale);
                        let x = place(side);
                        scene.icon_button(
                            view,
                            settings,
                            &control,
                            if view.revealed() {
                                Emblem::EyeOff
                            } else {
                                Emblem::Eye
                            },
                            Rect {
                                x,
                                y: field.y + (field.height - side) / 2.,
                                width: side,
                                height: side,
                            },
                            scale,
                        );
                        place(reveal_width - side);
                    }
                    if caps_badge {
                        let side = 18. * scale;
                        let x = place(caps_width);
                        scene.icon(
                            Emblem::CapsLock,
                            Rect {
                                x: x + (caps_width - side) / 2.,
                                y: field.y + (field.height - side) / 2.,
                                width: side,
                                height: side,
                            },
                            if look.dark {
                                palette.warning
                            } else {
                                super::raster::mix(palette.warning, palette.text, 0.45)
                            },
                        );
                    }
                }
                Content::Pill(control) => {
                    let pill = Rect {
                        height: FIELD * scale,
                        ..row
                    };
                    scene.button(view, settings, control, pill, scale, false);
                    if let Some(angle) = scene.spinner {
                        let side = 22. * scale;
                        scene.spinner(
                            Rect {
                                x: if rtl {
                                    pill.x + 16. * scale
                                } else {
                                    pill.x + pill.width - 16. * scale - side
                                },
                                y: pill.y + (pill.height - side) / 2.,
                                width: side,
                                height: side,
                            },
                            angle,
                            palette.accent,
                            look.track(),
                        );
                    }
                }
                Content::Links(controls) => {
                    self.populate_links(view, settings, scene, controls, left, width, top + 2. * scale, scale);
                }
                Content::Buttons(controls, treatment) => {
                    let height = self.row_height(controls, *treatment);
                    if *treatment == Treatment::Choice
                        && (self.panel.list_actions || self.card)
                        && controls.iter().all(|control| control.kind != ControlKind::Back)
                    {
                        for control in controls {
                            self.populate_row(
                                view,
                                settings,
                                scene,
                                control,
                                Rect {
                                    height: height * scale,
                                    ..row
                                },
                                scale,
                            );
                        }
                    } else {
                        self.populate_buttons(
                            view,
                            settings,
                            scene,
                            controls,
                            Rect {
                                height: height * scale,
                                ..row
                            },
                            scale,
                            *treatment == Treatment::Utility,
                        );
                    }
                }
                Content::System => self.populate_system(view, settings, scene, row, scale, true),
            }
        }
    }

    /// Quiet account actions, centred on one line.
    #[allow(clippy::too_many_arguments)]
    fn populate_links(
        &self,
        view: &SessionView,
        settings: &crate::settings::Settings,
        scene: &mut Scene,
        controls: &[&Control],
        left: f32,
        width: f32,
        y: f32,
        scale: f32,
    ) {
        let gap = 8. * scale;
        let label = |control: &Control| match &control.kind {
            ControlKind::AccountChooser { text } => text.clone(),
            _ => control.label.clone(),
        };
        let widths: Vec<_> = controls
            .iter()
            .map(|control| (view.text_width(&label(control), 14. * scale) + 70. * scale).min(width))
            .collect();
        let total = widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32;
        let mut x = left + (width - total) / 2.;
        for (control, w) in controls.iter().zip(widths) {
            let text = label(control);
            scene.styled_button(
                settings,
                control,
                Rect {
                    x,
                    y,
                    width: w,
                    height: 32. * scale,
                },
                scale,
                // A ghost pill: plainly a button, quieter than the field above it.
                Kind::Glass,
                16. * scale,
                ButtonContent::Chip {
                    icon: Some(Emblem::Accounts),
                    label: &text,
                    trailing: None,
                },
            );
            x += w + gap;
        }
    }

    fn row_height(&self, controls: &[&Control], treatment: Treatment) -> f32 {
        if self.card && treatment == Treatment::Choice {
            return 68.;
        }
        if controls
            .iter()
            .any(|control| matches!(control.kind, ControlKind::Tile(_)))
        {
            TILE
        } else {
            self.panel.control_height(treatment)
        }
    }

    /// One choice in a chooser list: picture, title, detail and a check mark.
    #[allow(clippy::too_many_arguments)]
    fn populate_row(
        &self,
        view: &SessionView,
        settings: &crate::settings::Settings,
        scene: &mut Scene,
        control: &Control,
        rect: Rect,
        scale: f32,
    ) {
        let checked = self.panel.checked_control == Some(control.id);
        // The current choice says so in words as well as with its check mark.
        let current = fl!("session-current");
        let described = self.panel.choice_details.get(&control.id).map(|detail| {
            if checked {
                format!("{current} · {detail}")
            } else {
                detail.clone()
            }
        });
        let detail = described
            .as_deref()
            .or(checked.then_some(current.as_str()));
        let (title, detail, leading) = match &control.kind {
            ControlKind::AccountChooser { text } => {
                (text.as_str(), None, Some(Leading::Icon(Emblem::Person)))
            }
            ControlKind::Account {
                name,
                detail: account,
                initials,
            } => (
                name.as_str(),
                account.as_deref().or(detail),
                // The highlighted account shows its picture once loaded.
                Some(
                    if control.selected && (view.user_avatar.is_some() || view.avatar.is_some()) {
                        Leading::Avatar(view)
                    } else {
                        Leading::Initials(initials)
                    },
                ),
            ),
            _ if self.panel.emblem == Emblem::Accounts => {
                return self.populate_account_row(settings, scene, control, rect, scale, checked, detail);
            }
            _ => (
                control.label.as_str(),
                detail,
                Some(Leading::Icon(
                    if self.panel.emblem == Emblem::Display && is_recovery(&control.label) {
                        Emblem::Lifebuoy
                    } else {
                        self.panel.emblem
                    },
                )),
            ),
        };
        scene.styled_button(
            settings,
            control,
            rect,
            scale,
            Kind::Row { checked },
            14. * scale,
            ButtonContent::Row {
                leading,
                title,
                detail,
                checked,
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn populate_account_row(
        &self,
        settings: &crate::settings::Settings,
        scene: &mut Scene,
        control: &Control,
        rect: Rect,
        scale: f32,
        checked: bool,
        detail: Option<&str>,
    ) {
        let monogram = initials(&control.label);
        scene.styled_button(
            settings,
            control,
            rect,
            scale,
            Kind::Row { checked },
            14. * scale,
            ButtonContent::Row {
                leading: Some(Leading::Initials(&monogram)),
                title: &control.label,
                detail,
                checked,
            },
        );
    }

    /// A dialog's footer: the disclosure at the start, then Cancel and the
    /// default action at the end.
    fn populate_footer(
        &self,
        view: &SessionView,
        settings: &crate::settings::Settings,
        scene: &mut Scene,
        row: Rect,
        scale: f32,
    ) {
        let rtl = canopa_i18n::direction().is_rtl();
        let gap = 10. * scale;
        let fit = |control: &Control| {
            // Wide enough that a changing label ("Checking…") never moves Cancel.
            (view.text_width(&control.label, 14. * scale) + 40. * scale).max(140. * scale)
        };
        let mut widths: Vec<f32> = self.footer.iter().map(|control| fit(control)).collect();
        let disclosure_width = self
            .footer_disclosure
            .map(|control| view.text_width(&control.label, 14. * scale) + 64. * scale);
        let available = row.width - disclosure_width.map_or(0., |w| w + gap);
        let total = widths.iter().sum::<f32>() + gap * widths.len().saturating_sub(1) as f32;
        if total > available {
            let each = (available - gap * widths.len().saturating_sub(1) as f32)
                / widths.len().max(1) as f32;
            widths.iter_mut().for_each(|w| *w = each);
        }
        // The default action is last, at the trailing edge.
        let mut edge = if rtl { row.x } else { row.x + row.width };
        for (control, width) in self.footer.iter().zip(widths).rev() {
            let x = if rtl {
                let x = edge;
                edge += width + gap;
                x
            } else {
                edge -= width;
                let x = edge;
                edge -= gap;
                x
            };
            let rect = Rect { x, width, ..row };
            if let Some(angle) = scene
                .spinner
                .filter(|_| view.primary_control == Some(control.id) && !control.enabled)
            {
                // The default action shows the work in its own place, as the
                // full-screen surfaces do: a tonal pill, spinner and label centred.
                let look = Look::new(settings);
                scene.surface(
                    rect,
                    10. * scale,
                    style::rgba(look.palette.accent, 1.),
                    iced::Color::TRANSPARENT,
                    0.,
                    iced::Shadow::default(),
                );
                let side = 16. * scale;
                let label = view.text_width(&control.label, 14. * scale);
                let group = side + 8. * scale + label;
                let start = rect.x + (rect.width - group) / 2.;
                let (spinner_x, label_x) = if rtl {
                    (start + label + 8. * scale, start)
                } else {
                    (start, start + side + 8. * scale)
                };
                scene.spinner(
                    Rect {
                        x: spinner_x,
                        y: rect.y + (rect.height - side) / 2.,
                        width: side,
                        height: side,
                    },
                    angle,
                    look.on_accent,
                    style::rgba(look.on_accent, 0.25),
                );
                scene.label(
                    control.label.clone(),
                    Rect {
                        x: label_x,
                        width: label + 4. * scale,
                        ..rect
                    },
                    14. * scale,
                    look.on_accent,
                    false,
                    Weight::Medium,
                );
            } else {
                scene.button(view, settings, control, rect, scale, false);
            }
        }
        // A visible field or the default action carries the spinner itself.
        if let Some(angle) = scene.spinner.filter(|_| {
            (view.prompt.is_none() || view.hide_input) && view.primary_control.is_none()
        }) {
            let side = 22. * scale;
            let x = if rtl { edge } else { edge - side };
            scene.spinner(
                Rect {
                    x,
                    y: row.y + (row.height - side) / 2.,
                    width: side,
                    height: side,
                },
                angle,
                Look::new(settings).palette.accent,
                Look::new(settings).track(),
            );
        }
        if let (Some(control), Some(width)) = (self.footer_disclosure, disclosure_width) {
            let width = width.min(row.width);
            let back = control.kind == ControlKind::Back;
            scene.styled_button(
                settings,
                control,
                Rect {
                    x: if rtl {
                        row.x + row.width - width
                    } else if back {
                        row.x
                    } else {
                        row.x
                    },
                    width,
                    ..row
                },
                scale,
                // Going back weighs the same as Cancel; details are an outlined
                // secondary action, lighter than the footer's buttons.
                if back { Kind::Secondary } else { Kind::Outline },
                10. * scale,
                if control.kind == ControlKind::Back {
                    ButtonContent::Chip {
                        icon: Some(Emblem::ArrowBack),
                        label: &control.label,
                        trailing: None,
                    }
                } else {
                    ButtonContent::Chip {
                        icon: None,
                        label: &control.label,
                        trailing: Some(Emblem::Chevron),
                    }
                },
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn populate_request(
        &self,
        view: &SessionView,
        settings: &crate::settings::Settings,
        scene: &mut Scene,
        request: &Request<'_>,
        row: Rect,
        scale: f32,
    ) {
        let look = Look::new(settings);
        let palette = look.palette;
        let rtl = canopa_i18n::direction().is_rtl();
        let (left, width, top) = (row.x, row.width, row.y);
        let height = request.height() * scale;
        scene.surface(
            Rect { height, ..row },
            14. * scale,
            style::rgba(palette.raised, if look.dark { 0.75 } else { 0.7 }),
            look.hairline(),
            scale,
            iced::Shadow::default(),
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
                        13.5 * scale,
                        palette.muted,
                        false,
                        Weight::Normal,
                    );
                }
                base += KEY_IDENTITY * scale;
                scene.surface(
                    Rect {
                        y: base,
                        height: scale.max(1.),
                        ..area
                    },
                    0.,
                    look.hairline(),
                    iced::Color::TRANSPARENT,
                    0.,
                    iced::Shadow::default(),
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
            return;
        }
        let mut y = top + 12. * scale;
        if let Some(subject) = request.subject {
            scene.label(
                subject,
                Rect {
                    x: left + 16. * scale,
                    y,
                    width: width - 32. * scale,
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
                    x: left + 16. * scale,
                    y,
                    width: width - 32. * scale,
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
            if request.subject.is_some() || !request.lines.is_empty() {
                y += 2. * scale;
            }
            let side = 40. * scale;
            let avatar = Rect {
                x: if rtl {
                    left + width - 14. * scale - side
                } else {
                    left + 14. * scale
                },
                y: y + 2. * scale,
                width: side,
                height: side,
            };
            if view.user_avatar.is_some() || view.avatar.is_some() {
                scene.avatar(view, avatar, settings);
            } else {
                scene.initials(&initials(account), avatar, settings);
            }
            let text_x = left + if rtl { 14. } else { 68. } * scale;
            let text_width = width - 82. * scale;
            // The card names the account that authenticates; who asked is the
            // dialog's subtitle.
            let _ = request.context;
            // The consumer may say what kind of account is needed
            // ("Administrator permission required").
            scene.label(
                view.custom
                    .get("account-caption")
                    .cloned()
                    .unwrap_or_else(|| fl!("session-authenticating-as")),
                Rect {
                    x: text_x,
                    y: y + 2. * scale,
                    width: text_width,
                    height: 18. * scale,
                },
                13. * scale,
                palette.muted,
                false,
                Weight::Medium,
            );
            scene.label(
                account,
                Rect {
                    x: text_x,
                    y: y + 19. * scale,
                    width: text_width,
                    height: 24. * scale,
                },
                15. * scale,
                palette.text,
                false,
                Weight::Semibold,
            );
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
        let staged = (scene.alpha, scene.offset);
        (scene.alpha, scene.offset) = scene.base;
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
        (scene.alpha, scene.offset) = staged;
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
            scene.button_detail(
                view,
                settings,
                control,
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
        let look = Look::new(settings);
        let palette = look.palette;
        let rtl = canopa_i18n::direction().is_rtl();
        let mut chips = Vec::new();
        for item in &self.system {
            let (control, label, icon, start, level, trailing) = match item {
                SystemItem::Desktop(c) => {
                    // "Desktop: Canopa" says what the chooser changes.
                    let label = c.label.clone();
                    (Some(*c), label, Emblem::Display, true, None, Some(Emblem::ChevronDown))
                }
                SystemItem::Keyboard(c) => (
                    *c,
                    view.layout.clone(),
                    Emblem::Keyboard,
                    false,
                    None,
                    c.map(|_| Emblem::ChevronDown),
                ),
                SystemItem::Power(c) => (Some(*c), String::new(), Emblem::Power, false, None, None),
                SystemItem::Status(icon, label, level) => {
                    (None, label.clone(), *icon, false, *level, None)
                }
            };
            let width = if label.is_empty() {
                CHIP * scale
            } else {
                (view.text_width(&label, 14. * scale)
                    + if trailing.is_some() { 74. } else { 58. } * scale)
                    .min(260. * scale)
            };
            chips.push((control, label, icon, start, width, level, trailing));
        }
        let total = chips.iter().map(|c| c.4).sum::<f32>()
            + 10. * scale * chips.len().saturating_sub(1) as f32;
        let mut left = if centered {
            row.x + (row.width - total) / 2.
        } else {
            row.x
        };
        let mut right = row.x + row.width;
        for (control, label, icon, start, width, level, trailing) in chips {
            let from_start = if rtl { !start } else { start };
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
            // While the system checks a response, every session control waits.
            let waiting;
            // While a response is checked, every session control waits.
            let control = if view.busy && self.mode == Mode::Form {
                waiting = control.map(|c| Control {
                    enabled: false,
                    ..c.clone()
                });
                waiting.as_ref()
            } else {
                control
            };
            if let Some(control) = control {
                if label.is_empty() {
                    scene.icon_button(view, settings, control, icon, rect, scale);
                } else {
                    // Selectors are filled pills; switching account is a quiet action.
                    scene.styled_button(
                        settings,
                        control,
                        rect,
                        scale,
                        if matches!(control.kind, ControlKind::AccountChooser { .. }) {
                            Kind::Quiet
                        } else {
                            Kind::Glass
                        },
                        rect.height / 2.,
                        ButtonContent::Chip {
                            icon: Some(icon),
                            label: &label,
                            trailing,
                        },
                    );
                }
            } else {
                // A passive status reads as status, not as a control: no pill.
                let side = 20. * scale;
                let inner = Rect {
                    x: rect.x + 14. * scale,
                    width: rect.width - 28. * scale,
                    ..rect
                };
                scene.status_icon(
                    icon,
                    Rect {
                        x: if rtl {
                            inner.x + inner.width - side
                        } else {
                            inner.x
                        },
                        y: rect.y + (rect.height - side) / 2.,
                        width: side,
                        height: side,
                    },
                    palette.text,
                    level,
                );
                scene.label(
                    label,
                    Rect {
                        x: inner.x + if rtl { 0. } else { side + 8. * scale },
                        width: (inner.width - side - 8. * scale).max(1.),
                        ..rect
                    },
                    14. * scale,
                    palette.text,
                    false,
                    Weight::Medium,
                );
            }
        }
    }
}

/// The colour of a strength rating from -1 (rejected) to 1, in the meter's bands.
fn strength_color(level: f32, palette: &canopa_theme::Palette, warning: canopa_theme::Rgb) -> canopa_theme::Rgb {
    match level {
        level if level < 0. => palette.danger,
        level if level <= 0. => palette.muted,
        // Weak is advice, not an error; only a rejected passphrase is.
        level if level < 0.30 => warning,
        level if level < 0.60 => warning,
        level if level < 0.85 => palette.accent,
        _ => palette.success,
    }
}

/// A troubleshooting session, shown with its own icon.
fn is_recovery(label: &str) -> bool {
    let label = label.to_lowercase();
    ["recovery", "rescue", "safe", "failsafe", "secours", "dépannage"]
        .iter()
        .any(|word| label.contains(word))
}

/// Up to two initials for a display name.
pub fn initials(name: &str) -> String {
    let mut words = name.split_whitespace().filter_map(|word| word.chars().next());
    match (words.next(), words.last()) {
        (Some(first), Some(last)) => [first, last].iter().collect(),
        (Some(first), None) => first.to_string(),
        _ => String::new(),
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
            height += ACCOUNT_SECTION;
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
            facts: Vec::new(),
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
            card: false,
            checking: String::new(),
            checking_instruction: String::new(),
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
        let look = Look::new(&settings);
        expected.surface(
            Rect {
                x: 16.,
                y: 16.,
                width: 496.,
                height: plan.height - 32.,
            },
            14.,
            style::rgba(palette.raised, 0.75),
            look.hairline(),
            1.,
            iced::Shadow::default(),
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
            look.palette.muted,
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
