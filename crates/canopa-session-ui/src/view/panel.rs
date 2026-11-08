//! Shared presentation for the four session authentication surfaces.
use super::raster::{Emblem, blend, emblem, framed, mix, rounded, shadow};
use super::{Control, SessionView, wrap};
use crate::{layout::Rect, text::Style};
use canopa_theme::{Appearance, ColorRole, ColorScheme, Palette, Rgb};
use cosmic_text::Weight;
use image::RgbImage;

pub const DIALOG_SIZE: (u32, u32) = (560, 416);
pub const AUTHORIZATION_DIALOG_SIZE: (u32, u32) = (560, 392);
pub const DIALOG_MARGIN: f32 = 16.;

pub struct AuthenticationPanel {
    /// Reserve room in centered clock-and-form stacks without drawing extra card space.
    pub height_reserve: f32,
    pub heading: &'static str,
    pub eyebrow: &'static str,
    pub emblem: Emblem,
    pub dialog: bool,
    pub choices: &'static [(usize, Emblem)],
    pub utilities: &'static [usize],
    pub disclosure: Option<usize>,
    pub list_actions: bool,
    pub checked_control: Option<usize>,
    pub choice_details: std::collections::BTreeMap<usize, String>,
}

const PADDING: f32 = 28.;
const BUTTON: f32 = 44.;
const DIALOG_CONTENT_PADDING: f32 = 22.;
const DIALOG_FOOTER_PADDING: f32 = 24.;

enum Content<'a> {
    Heading,
    Copy(Vec<String>, bool),
    Request {
        subject: Option<&'a str>,
        lines: Vec<String>,
        metadata: Vec<String>,
        disclosure: Option<&'a Control>,
    },
    Divider,
    Field(Vec<String>),
    Buttons(Vec<&'a Control>, Treatment),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Treatment {
    Choice,
    Action,
    Utility,
}

pub(super) struct Plan<'a> {
    panel: &'a AuthenticationPanel,
    width: f32,
    height: f32,
    height_reserve: f32,
    username: bool,
    avatar: bool,
    keyboard_inline: bool,
    footer_disclosure: Option<&'a Control>,
    rows: Vec<(f32, Content<'a>)>,
}

impl AuthenticationPanel {
    fn control_height(&self, treatment: Treatment) -> f32 {
        match treatment {
            Treatment::Utility => 36.,
            Treatment::Choice if self.list_actions => 56.,
            Treatment::Choice if !self.dialog => 52.,
            _ => BUTTON,
        }
    }

    pub(super) fn plan<'a>(
        &'a self,
        view: &'a SessionView,
        keyboard: Option<&'a Control>,
        keyboard_inline: bool,
        username: bool,
        avatar: bool,
        pin_origin: bool,
    ) -> Plan<'a> {
        let width = match self.emblem {
            Emblem::Lock => 392.,
            _ if self.dialog => DIALOG_SIZE.0 as f32 - 2. * DIALOG_MARGIN,
            _ => 460.,
        };
        let mut plan = Plan {
            panel: self,
            width,
            height: if self.dialog {
                DIALOG_CONTENT_PADDING
            } else {
                PADDING
            },
            height_reserve: if self.dialog {
                let (_, height) = if matches!(self.emblem, Emblem::Shield) {
                    AUTHORIZATION_DIALOG_SIZE
                } else {
                    DIALOG_SIZE
                };
                height as f32 - 2. * DIALOG_MARGIN
            } else if pin_origin && self.height_reserve > 0. {
                self.height_reserve
                    + if keyboard.is_some() { 42. } else { 0. }
                    + if view.caps { 32. } else { 0. }
            } else {
                0.
            },
            username,
            avatar,
            keyboard_inline,
            footer_disclosure: None,
            rows: Vec::new(),
        };
        let mut choices = Vec::new();
        let mut actions = Vec::new();
        let mut utilities = Vec::new();
        let mut disclosure = None;
        let has_field = !view.hide_input && view.prompt.is_some();
        for control in &view.controls {
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
        if self.dialog
            && actions.len() == 2
            && actions
                .iter()
                .any(|control| view.primary_control == Some(control.id))
        {
            plan.footer_disclosure = disclosure;
        }
        let body_disclosure = disclosure.filter(|_| plan.footer_disclosure.is_none());
        let session_picker_in_form = !self.dialog
            && view
                .prompt
                .as_ref()
                .is_some_and(|prompt| prompt.id == 0 && prompt.echo)
            && choices
                .iter()
                .any(|control| control.label.starts_with("Desktop: "));
        plan.push(Content::Heading, 0., 52.);
        if self.dialog {
            let lines: Vec<_> = ["message-0", "message-1"]
                .iter()
                .filter_map(|key| view.custom.get(*key).filter(|s| !s.is_empty()))
                .flat_map(|text| wrap(text, 60, 8))
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
            let mut metadata = Vec::new();
            if let Some(account) = view.custom.get("account") {
                metadata.push(account.clone());
            }
            if disclosure.is_some()
                && (!lines.is_empty() || subject.is_some() || !metadata.is_empty())
            {
                let height = request_height(
                    &lines,
                    subject.is_some(),
                    metadata.len(),
                    !matches!(self.emblem, Emblem::Shield),
                );
                plan.push(
                    Content::Request {
                        subject,
                        lines,
                        metadata,
                        disclosure: body_disclosure,
                    },
                    12.,
                    height,
                );
            } else {
                plan.copy(lines, 16., false);
                utilities.extend(body_disclosure);
            }
            plan.buttons(&utilities, Treatment::Utility, 6., 36.);
            plan.buttons(&choices, Treatment::Choice, 12., BUTTON);
        }
        if has_field {
            let label = view.prompt.as_ref().map_or_else(Vec::new, |prompt| {
                wrap(prompt.text.trim_end_matches(':'), 56, 8)
            });
            let height = 52. + label.len().max(1) as f32 * 20.;
            plan.push(
                Content::Field(label),
                match self.emblem {
                    Emblem::Key => 18.,
                    _ if self.dialog => 10.,
                    _ => 20.,
                },
                height,
            );
        }
        if view.hide_input
            && !self.list_actions
            && let Some(prompt) = &view.prompt
        {
            plan.copy(wrap(&prompt.text, 56, 8), 16., false);
        }
        if !view.message.is_empty() {
            plan.copy(wrap(&view.message, 57, 8), 12., true);
        }
        if view.caps {
            plan.copy(vec!["Caps Lock is on".into()], 10., true);
        }
        if session_picker_in_form {
            plan.buttons(
                &choices,
                Treatment::Choice,
                14.,
                self.control_height(Treatment::Choice),
            );
        }
        for (index, row) in actions
            .chunks(if self.list_actions { 1 } else { 2 })
            .enumerate()
        {
            let mut row = row.to_vec();
            if row.len() == 2 && view.primary_control == Some(row[0].id) {
                row.swap(0, 1);
            }
            let treatment = if self.list_actions {
                Treatment::Choice
            } else {
                Treatment::Action
            };
            plan.push(
                Content::Buttons(row, treatment),
                if self.dialog && index == 0 {
                    DIALOG_FOOTER_PADDING + 20.
                } else if self.list_actions && index > 0 {
                    8.
                } else {
                    16.
                },
                self.control_height(treatment),
            );
        }
        if !self.dialog {
            if !session_picker_in_form && !choices.is_empty() {
                plan.push(Content::Divider, 20., 1.);
                plan.buttons(
                    &choices,
                    Treatment::Choice,
                    12.,
                    self.control_height(Treatment::Choice),
                );
            }
            if self.list_actions && !utilities.is_empty() {
                plan.push(Content::Divider, 14., 1.);
            }
            plan.buttons(&utilities, Treatment::Utility, 8., 36.);
        }
        if let Some(keyboard) = keyboard.filter(|_| !keyboard_inline || !has_field) {
            plan.buttons(&[keyboard], Treatment::Utility, 6., 36.);
        }
        plan.height += if self.dialog {
            DIALOG_FOOTER_PADDING - DIALOG_MARGIN
        } else if self.list_actions {
            24.
        } else {
            PADDING
        };
        plan
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

    fn copy(&mut self, lines: Vec<String>, gap: f32, notice: bool) {
        if !lines.is_empty() {
            let height = 22. * lines.len() as f32;
            self.push(Content::Copy(lines, notice), gap, height);
        }
    }

    pub(super) fn size(&self) -> (f32, f32) {
        (self.width, self.height.max(self.height_reserve))
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
        let card = palette.surface;
        let edge = mix(
            card,
            palette.border,
            if matches!(self.panel.emblem, Emblem::Lock)
                && appearance.color_scheme == ColorScheme::Dark
            {
                0.86
            } else {
                0.67
            },
        );
        bounds.height = self.height * scale;
        if !self.panel.dialog {
            let strength = if appearance.color_scheme == ColorScheme::Light {
                0.055
            } else {
                0.18
            };
            shadow(image, bounds, 26. * scale, scale, strength);
            framed(image, bounds, 26. * scale, scale, card, edge);
        }

        let padding = if self.panel.dialog {
            DIALOG_CONTENT_PADDING
        } else {
            PADDING
        };
        let left = bounds.x + padding * scale;
        let width = bounds.width - 2. * padding * scale;
        let spare = (self.height_reserve - self.height).max(0.);
        let footer_offset = self
            .panel
            .dialog
            .then(|| {
                self.rows.iter().find_map(|(offset, row)| {
                    matches!(row, Content::Buttons(_, Treatment::Action)).then_some(*offset)
                })
            })
            .flatten();
        if let Some(offset) = footer_offset {
            let y = bounds.y + (offset + spare - DIALOG_FOOTER_PADDING) * scale;
            rounded(
                image,
                Rect {
                    x: left,
                    y,
                    width,
                    height: scale,
                },
                0.,
                mix(card, palette.border, 0.55),
                1.,
            );
        }
        let mut hits = Vec::new();
        let mut after_field = false;
        for (offset, row) in &self.rows {
            after_field |= matches!(row, Content::Field(_));
            let shift = match footer_offset {
                Some(footer) if *offset >= footer => spare,
                Some(_) if after_field => spare,
                Some(_) if matches!(row, Content::Request { .. }) => spare / 2.,
                _ => 0.,
            };
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
            };
            match row {
                Content::Heading => paint.heading(self.panel, self.username, self.avatar),
                Content::Request {
                    subject,
                    lines,
                    metadata,
                    disclosure,
                } => {
                    let policy = matches!(self.panel.emblem, Emblem::Shield);
                    paint.request(*subject, lines, metadata, *disclosure, policy, &mut hits);
                }
                Content::Divider => {
                    let line = Rect {
                        x: left,
                        y: top,
                        width,
                        height: scale,
                    };
                    rounded(paint.image, line, 0., mix(card, edge, 0.58), 1.);
                }
                Content::Copy(lines, notice) => {
                    let ink = if *notice && view.failed {
                        palette.danger
                    } else if *notice && view.caps && lines[0] == "Caps Lock is on" {
                        palette.warning
                    } else {
                        palette.muted
                    };
                    for (i, line) in lines.iter().enumerate() {
                        paint.label(
                            line,
                            (0., 16. + i as f32 * 22.),
                            15.,
                            width,
                            ink,
                            Weight::NORMAL,
                        );
                    }
                }
                Content::Field(label) => paint.field(label, self.keyboard_inline, &mut hits),
                Content::Buttons(controls, treatment) => {
                    paint.buttons(controls, *treatment, self.panel, &mut hits);
                    if footer_offset == Some(*offset)
                        && let Some(disclosure) = self.footer_disclosure
                    {
                        paint.disclosure(disclosure, 0., true, &mut hits);
                    }
                }
            }
        }
        hits
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
}

impl Paint<'_> {
    fn label(
        &mut self,
        value: &str,
        (x, y): (f32, f32),
        size: f32,
        width: f32,
        color: Rgb,
        weight: Weight,
    ) {
        let text: String = value
            .chars()
            .filter(|c| !c.is_control())
            .take(256)
            .collect();
        self.view.text_renderer.borrow_mut().draw_styled(
            self.image,
            &text,
            (self.left + x * self.scale, self.top + y * self.scale),
            size * self.scale,
            width,
            color,
            None,
            Style::Leading(weight),
        );
    }

    fn heading(&mut self, panel: &AuthenticationPanel, username: bool, show_avatar: bool) {
        let s = self.scale;
        let tile = Rect {
            x: self.left,
            y: self.top,
            width: 52. * s,
            height: 52. * s,
        };
        let authority = matches!(panel.emblem, Emblem::Shield);
        let accent_surface = if authority {
            mix(self.palette.surface, self.palette.warning, 0.15)
        } else {
            self.appearance.role(ColorRole::AccentSurface)
        };
        let focus = self.appearance.role(ColorRole::Focus);
        let tone = if authority {
            self.palette.warning
        } else {
            self.palette.accent
        };
        let tint = if tone.contrast(accent_surface) >= 4.5 {
            tone
        } else {
            self.palette.text
        };
        rounded(self.image, tile, 15. * s, accent_surface, 1.);
        let show_avatar = show_avatar && !panel.list_actions;
        let locked_identity = username && matches!(panel.emblem, Emblem::Lock);
        let initials = self.view.avatar_initials.as_deref().filter(|_| show_avatar);
        if !(show_avatar && self.avatar(tile, 15. * s)) {
            if let Some(initials) = initials {
                self.view.text_renderer.borrow_mut().draw_styled(
                    self.image,
                    initials,
                    (tile.x + 26. * s, tile.y + 33. * s),
                    19. * s,
                    tile.width - 8. * s,
                    tint,
                    None,
                    Style::Centered(Weight::SEMIBOLD),
                );
            } else {
                let size = if matches!(panel.emblem, Emblem::Lock) {
                    1.30
                } else {
                    1.
                };
                let inset = (52. - 24. * size) / 2. * s;
                emblem(
                    self.image,
                    (tile.x + inset, tile.y + inset),
                    size * s,
                    panel.emblem,
                    tint,
                );
            }
        }
        let eyebrow = self
            .view
            .custom
            .get("heading")
            .map_or_else(|| panel.eyebrow.into(), |heading| heading.to_uppercase());
        let heading = if username {
            self.view.username.as_str()
        } else {
            self.view
                .custom
                .get("request-title")
                .map_or(panel.heading, String::as_str)
        };
        let width = self.width - 68. * s;
        if locked_identity {
            self.label(
                "Session locked",
                (68., 24.),
                26.,
                width,
                self.palette.text,
                Weight::SEMIBOLD,
            );
            self.label(
                heading,
                (68., 48.),
                16.,
                width,
                mix(self.palette.muted, self.palette.text, 0.47),
                Weight::MEDIUM,
            );
        } else {
            self.label(
                &eyebrow,
                (68., 15.),
                12.,
                width,
                if authority && tone.contrast(self.palette.surface) >= 4.5 {
                    tone
                } else {
                    focus
                },
                Weight::SEMIBOLD,
            );
            self.label(
                heading,
                (68., 44.),
                28.,
                width,
                self.palette.text,
                Weight::SEMIBOLD,
            );
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

    fn keyboard_status(&mut self, baseline: f32, hits: &mut Vec<(usize, Rect)>) {
        let s = self.scale;
        let label = if self.view.can_select_layout {
            format!("{} · F6", self.view.layout)
        } else {
            self.view.layout.clone()
        };
        let width = self.view.text_width(&label, 15. * s).min(120. * s);
        let rect = Rect {
            x: self.left + self.width - width - 29. * s,
            y: self.top + (baseline - 19.) * s,
            width: width + 34. * s,
            height: 28. * s,
        };
        let ink = if self.view.can_select_layout {
            hits.push((crate::input::LAYOUT_CONTROL, rect));
            self.appearance.role(ColorRole::Focus)
        } else {
            self.palette.muted
        };
        emblem(
            self.image,
            (rect.x, rect.y + 7. * s),
            s * 0.75,
            Emblem::Keyboard,
            ink,
        );
        self.label(
            &label,
            ((rect.x - self.left) / s + 25., baseline),
            15.,
            width + 3. * s,
            ink,
            Weight::MEDIUM,
        );
    }

    fn disclosure(
        &mut self,
        control: &Control,
        inset: f32,
        leading: bool,
        hits: &mut Vec<(usize, Rect)>,
    ) {
        let s = self.scale;
        let height = if leading { BUTTON } else { 34. };
        let link = Rect {
            x: if leading {
                self.left
            } else {
                self.left + self.width - 138. * s
            },
            y: self.top + inset * s,
            width: 126. * s,
            height: height * s,
        };
        let focus = self.appearance.role(ColorRole::Focus);
        let (label, selected) = control
            .label
            .strip_prefix("› ")
            .map_or((control.label.as_str(), false), |label| (label, true));
        let button = mix(
            self.palette.surface,
            self.palette.raised,
            if leading { 0.54 } else { 0.72 },
        );
        let border = if selected {
            focus
        } else if leading {
            mix(button, focus, 0.34)
        } else {
            mix(button, self.palette.border, 0.68)
        };
        framed(self.image, link, 10. * s, s, button, border);
        self.label(
            label,
            ((link.x - self.left) / s + 10., inset + height / 2. + 5.),
            14.,
            98. * s,
            focus,
            Weight::MEDIUM,
        );
        emblem(
            self.image,
            (link.x + 102. * s, link.y + (height / 2. - 8.) * s),
            s * 2. / 3.,
            Emblem::Chevron,
            focus,
        );
        hits.push((control.id, link));
    }

    fn request(
        &mut self,
        subject: Option<&str>,
        lines: &[String],
        metadata: &[String],
        disclosure: Option<&Control>,
        policy: bool,
        hits: &mut Vec<(usize, Rect)>,
    ) {
        let s = self.scale;
        let height = request_height(lines, subject.is_some(), metadata.len(), !policy) * s;
        let identity_only = policy
            && lines.is_empty()
            && metadata.iter().any(|value| value.starts_with("Account: "));
        let context = if policy && disclosure.is_none() {
            self.view
                .custom
                .get("policy-publisher")
                .map(|publisher| ("Policy source", publisher))
                .or_else(|| {
                    self.view
                        .custom
                        .get("subject-process")
                        .map(|process| ("Requested for", process))
                })
        } else {
            None
        };
        let fingerprint = (!policy).then(|| fingerprint_groups(lines)).flatten();
        let rail = if policy {
            self.palette.warning
        } else {
            self.appearance.role(ColorRole::Focus)
        };
        rounded(
            self.image,
            Rect {
                x: self.left,
                y: self.top,
                width: self.width,
                height,
            },
            12. * s,
            mix(self.palette.surface, self.palette.raised, 0.57),
            1.,
        );
        rounded(
            self.image,
            Rect {
                x: self.left,
                y: self.top + 8. * s,
                width: 2. * s,
                height: (height - 16. * s).max(s),
            },
            s,
            mix(self.palette.surface, rail, 0.63),
            1.,
        );
        if disclosure.is_none()
            && let Some(fingerprint) = fingerprint
        {
            self.key_identity(subject, fingerprint);
            return;
        }
        let line_width = self.width - 32. * s;
        self.label(
            if identity_only {
                "Authenticating as"
            } else if policy {
                "POLICY REQUEST"
            } else if fingerprint.is_some() {
                "KEY IDENTITY"
            } else {
                "REQUEST CONTEXT"
            },
            (
                if identity_only { 62. } else { 16. },
                if identity_only { 26. } else { 24. },
            ),
            13.,
            line_width - if identity_only { 46. * s } else { 0. },
            self.palette.muted,
            Weight::SEMIBOLD,
        );
        if let Some(control) = disclosure {
            self.disclosure(
                control,
                if identity_only {
                    17.
                } else if policy {
                    6.
                } else {
                    0.
                },
                false,
                hits,
            );
        }
        let mut baseline = if identity_only {
            48.
        } else if !policy {
            46.
        } else if lines.is_empty() && subject.is_none() {
            49.
        } else {
            64.
        };
        if let Some(subject) = subject {
            self.label(
                subject,
                (16., baseline),
                16.,
                line_width,
                self.palette.text,
                Weight::SEMIBOLD,
            );
            baseline += if policy { 23. } else { 27. };
        }
        if let Some(fingerprint) = fingerprint {
            self.label(
                "Fingerprint",
                (16., baseline),
                12.,
                line_width,
                self.palette.muted,
                Weight::SEMIBOLD,
            );
            baseline += 21.;
            self.request_code(&fingerprint.join(" "), 16., baseline, line_width);
        }
        for line in lines.iter().filter(|_| fingerprint.is_none()) {
            let groups: Vec<_> = line.split_ascii_whitespace().collect();
            if !policy && hex_groups(&groups) {
                self.request_code(line, 16., baseline, line_width);
            } else {
                self.label(
                    line,
                    (16., baseline),
                    if policy { 16. } else { 15. },
                    line_width,
                    self.palette.text,
                    if policy {
                        Weight::SEMIBOLD
                    } else {
                        Weight::NORMAL
                    },
                );
            }
            baseline += 21.;
        }
        for (index, value) in metadata.iter().enumerate() {
            let account = value.strip_prefix("Account: ");
            let baseline = baseline + 2. + index as f32 * 20.;
            let mut text_inset = 16.;
            if let Some(name) = account.filter(|_| policy) {
                let (side, center, radius) = if identity_only {
                    (34., 35., 10.)
                } else {
                    (22., baseline - 5., 6.)
                };
                let tile = Rect {
                    x: self.left + 16. * s,
                    y: self.top + (center - side / 2.) * s,
                    width: side * s,
                    height: side * s,
                };
                if !self.avatar(tile, radius * s) {
                    self.initial_tile(name, 16., center, side, false);
                }
                text_inset += side + 12.;
            }
            let available = if context.is_some() {
                self.width / 2.
            } else {
                self.width
            } - (text_inset + 16.) * s;
            let available = if identity_only && disclosure.is_some() {
                available - 140. * s
            } else {
                available
            };
            self.label(
                if identity_only {
                    account.unwrap_or(value)
                } else {
                    value
                },
                (text_inset, baseline),
                if identity_only { 16. } else { 13. },
                available,
                if identity_only {
                    self.palette.text
                } else {
                    self.palette.muted
                },
                Weight::MEDIUM,
            );
        }
        if let Some((label, value)) = context {
            let x = self.width / s / 2. + 16.;
            let width = self.width / 2. - 32. * s;
            if identity_only {
                rounded(
                    self.image,
                    Rect {
                        x: self.left + self.width / 2.,
                        y: self.top + 16. * s,
                        width: s,
                        height: height - 32. * s,
                    },
                    0.,
                    self.palette.border,
                    0.53,
                );
                self.label(
                    label,
                    (x, 26.),
                    13.,
                    width,
                    self.palette.muted,
                    Weight::SEMIBOLD,
                );
                self.label(
                    value,
                    (x, 50.),
                    16.,
                    width,
                    self.palette.text,
                    Weight::MEDIUM,
                );
            } else {
                self.label(
                    &format!("{label}: {value}"),
                    (x, baseline + 2.),
                    13.,
                    width,
                    self.palette.muted,
                    Weight::MEDIUM,
                );
            }
        }
    }

    fn request_code(&mut self, value: &str, x: f32, baseline: f32, width: f32) {
        let text = value.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
        let s = self.scale;
        self.view.text_renderer.borrow_mut().draw_styled(
            self.image,
            &text,
            (self.left + x * s, self.top + baseline * s),
            15. * s,
            width,
            mix(self.palette.text, self.palette.muted, 0.24),
            None,
            Style::Code,
        );
    }

    fn key_identity(&mut self, subject: Option<&str>, fingerprint: &[String]) {
        let s = self.scale;
        let baseline = if subject.is_some() { 62. } else { 55. };
        let mut fingerprint_x = 16.;
        if let Some(subject) = subject {
            let split = (self.width / s * 0.43).round();
            fingerprint_x = split + 17.;
            let divider = Rect {
                x: self.left + split * s,
                y: self.top + 19. * s,
                width: s,
                height: 72. * s,
            };
            rounded(
                self.image,
                divider,
                0.,
                self.palette.border,
                if self.appearance.color_scheme == ColorScheme::Light {
                    0.78
                } else {
                    0.57
                },
            );
            let width = (split - 31.) * s;
            self.label(
                "Key identity",
                (16., 37.),
                13.,
                width,
                self.palette.muted,
                Weight::SEMIBOLD,
            );
            let (name, detail) = subject
                .split_once(" · ")
                .map_or((subject, None), |(name, detail)| (name, Some(detail)));
            let lines = wrap(name, 24, 2);
            for (index, line) in lines.iter().enumerate() {
                self.label(
                    line,
                    (16., baseline + index as f32 * 21.),
                    17.,
                    width,
                    self.palette.text,
                    Weight::SEMIBOLD,
                );
            }
            if let Some(detail) = detail.filter(|_| lines.len() == 1) {
                self.label(
                    detail,
                    (16., baseline + 21.),
                    15.,
                    width,
                    self.palette.muted,
                    Weight::NORMAL,
                );
            }
        }
        let width = self.width - (fingerprint_x + 16.) * s;
        self.label(
            "Fingerprint",
            (fingerprint_x, baseline - 25.),
            14.,
            width,
            mix(self.palette.muted, self.palette.text, 0.35),
            Weight::SEMIBOLD,
        );
        for (index, line) in fingerprint.iter().enumerate() {
            self.request_code(line, fingerprint_x, baseline + index as f32 * 21., width);
        }
    }

    fn initial_tile(&mut self, name: &str, x: f32, center: f32, size: f32, selected: bool) {
        let s = self.scale;
        let tile = Rect {
            x: self.left + x * s,
            y: self.top + (center - size / 2.) * s,
            width: size * s,
            height: size * s,
        };
        let fill = if selected {
            mix(
                self.palette.surface,
                self.appearance.role(ColorRole::AccentSurface),
                0.42,
            )
        } else {
            self.appearance.role(ColorRole::AccentSurface)
        };
        let ink = if self.palette.accent.contrast(fill) >= 4.5 {
            self.palette.accent
        } else {
            self.palette.text
        };
        let mut words = name
            .split_whitespace()
            .filter_map(|word| word.chars().find(|c| c.is_alphanumeric()));
        let first = words.next();
        let last = words.next_back();
        let initials: String = first
            .into_iter()
            .chain(last)
            .flat_map(char::to_uppercase)
            .collect();
        rounded(self.image, tile, 10. * s, fill, 1.);
        if initials.is_empty() {
            emblem(
                self.image,
                (tile.x + (size / 2. - 9.) * s, tile.y + (size / 2. - 9.) * s),
                0.75 * s,
                Emblem::Accounts,
                ink,
            );
        } else {
            self.view.text_renderer.borrow_mut().draw_styled(
                self.image,
                &initials,
                (tile.x + size * s / 2., self.top + (center + 4.) * s),
                12. * s,
                (size - 6.) * s,
                ink,
                None,
                Style::Centered(Weight::SEMIBOLD),
            );
        }
    }

    fn field(&mut self, label: &[String], keyboard_inline: bool, hits: &mut Vec<(usize, Rect)>) {
        let s = self.scale;
        for (i, line) in label.iter().enumerate() {
            self.label(
                line,
                (0., 12. + i as f32 * 20.),
                14.,
                self.width
                    - if keyboard_inline && i == 0 {
                        146. * s
                    } else {
                        0.
                    },
                self.palette.text,
                Weight::MEDIUM,
            );
        }
        if keyboard_inline {
            self.keyboard_status(12., hits);
        }
        let y = self.top + label.len().max(1) as f32 * 20. * s;
        let field = Rect {
            x: self.left,
            y,
            width: self.width,
            height: 52. * s,
        };
        let focus = self.appearance.role(ColorRole::Focus);
        framed(
            self.image,
            field,
            12. * s,
            1.75 * s,
            if self.appearance.color_scheme == ColorScheme::Light {
                mix(self.palette.surface, self.palette.background, 0.78)
            } else {
                self.palette.background
            },
            focus,
        );

        let prompt = self
            .view
            .prompt
            .as_ref()
            .expect("visible field has a prompt");
        let (input, selection, cursor) = self.view.secret.presentation(prompt.echo, 48);
        if self.view.secret.is_empty() {
            let caret = Rect {
                x: self.left + 15. * s,
                y: y + 16. * s,
                width: 1.5 * s,
                height: 20. * s,
            };
            rounded(self.image, caret, 0.75 * s, focus, 1.);
        } else {
            self.view.text_renderer.borrow_mut().draw_styled(
                self.image,
                input.as_ref(),
                (self.left + 15. * s, y + 32. * s),
                16. * s,
                field.width - 30. * s,
                self.palette.text,
                Some((selection, cursor, focus)),
                Style::Leading(Weight::NORMAL),
            );
        }
    }

    fn buttons(
        &mut self,
        controls: &[&Control],
        treatment: Treatment,
        panel: &AuthenticationPanel,
        hits: &mut Vec<(usize, Rect)>,
    ) {
        let s = self.scale;
        let gap = 8. * s;
        let compact_actions = panel.dialog
            && treatment == Treatment::Action
            && controls.len() == 2
            && controls
                .iter()
                .any(|control| self.view.primary_control == Some(control.id));
        let row_width = if compact_actions {
            self.width.min(288. * s)
        } else {
            self.width
        };
        let row_left = self.left + self.width - row_width;
        let width = (row_width - gap * (controls.len() - 1) as f32) / controls.len() as f32;
        for (index, control) in controls.iter().enumerate() {
            let (label, selected) = control
                .label
                .strip_prefix("› ")
                .map_or((control.label.as_str(), false), |label| (label, true));
            let primary = self.view.primary_control == Some(control.id);
            let current = panel.list_actions && panel.checked_control == Some(control.id);
            let back = panel.list_actions && label.starts_with("← ");
            let rect = Rect {
                x: row_left + (width + gap) * index as f32,
                y: self.top,
                width: if back { width.min(139. * s) } else { width },
                height: panel.control_height(treatment) * s,
            };
            let fill = if primary {
                self.palette.accent
            } else if selected && (current || !panel.list_actions) {
                self.appearance.role(ColorRole::AccentSurface)
            } else if back {
                self.palette.surface
            } else if treatment == Treatment::Choice && !panel.dialog {
                mix(self.palette.surface, self.palette.raised, 0.7)
            } else if compact_actions {
                if self.appearance.color_scheme == ColorScheme::Dark {
                    mix(self.palette.raised, self.palette.text, 0.04)
                } else {
                    mix(self.palette.surface, self.palette.raised, 0.55)
                }
            } else {
                self.palette.raised
            };
            let ink = if primary {
                self.appearance.role(ColorRole::OnAccent)
            } else if treatment == Treatment::Utility && !selected {
                self.appearance.role(ColorRole::Focus)
            } else {
                self.palette.text
            };
            if treatment != Treatment::Utility || selected || back {
                let radius = 11. * s;
                let border = mix(
                    fill,
                    self.palette.border,
                    if compact_actions && self.appearance.color_scheme == ColorScheme::Dark {
                        0.88
                    } else {
                        0.58
                    },
                );
                if selected {
                    framed(
                        self.image,
                        rect,
                        radius,
                        1.5 * s,
                        fill,
                        self.appearance.role(ColorRole::Focus),
                    );
                } else if compact_actions && primary {
                    rounded(self.image, rect, radius, fill, 1.);
                } else {
                    framed(
                        self.image,
                        rect,
                        radius,
                        s,
                        fill,
                        if primary { fill } else { border },
                    );
                }
            }
            let size = if treatment == Treatment::Utility {
                13.
            } else {
                14.
            };
            if panel.list_actions && treatment == Treatment::Choice {
                let icon = panel.emblem;
                let title = label;
                let subtitle = panel.choice_details.get(&control.id);
                let center = rect.height / s / 2.;
                let offset = (rect.x - self.left) / s;
                let text_inset = 46.;
                if matches!(icon, Emblem::Accounts) {
                    self.initial_tile(title, offset + 10., center, 30., current);
                } else {
                    let tile = Rect {
                        x: rect.x + 10. * s,
                        y: rect.y + (center - 15.) * s,
                        width: 30. * s,
                        height: 30. * s,
                    };
                    let tint = self.appearance.role(ColorRole::AccentSurface);
                    let tint = if current {
                        mix(tint, self.palette.surface, 0.62)
                    } else {
                        tint
                    };
                    rounded(self.image, tile, 9. * s, tint, 1.);
                    emblem(
                        self.image,
                        (tile.x + 6. * s, tile.y + 6. * s),
                        0.75 * s,
                        Emblem::Display,
                        self.appearance.role(ColorRole::Focus),
                    );
                }
                self.label(
                    title,
                    (
                        offset + text_inset,
                        center + if subtitle.is_some() { -3. } else { 5. },
                    ),
                    15.,
                    width - if current { 140. * s } else { 84. * s },
                    ink,
                    Weight::SEMIBOLD,
                );
                if let Some(user) = subtitle {
                    self.label(
                        user,
                        (offset + 46., center + 17.),
                        14.,
                        width - if current { 140. * s } else { 84. * s },
                        mix(self.palette.muted, self.palette.text, 0.27),
                        Weight::NORMAL,
                    );
                }
                if current {
                    let chip = Rect {
                        x: rect.x + rect.width - 100. * s,
                        y: rect.y + (center - 13.) * s,
                        width: 87. * s,
                        height: 26. * s,
                    };
                    let focus = self.appearance.role(ColorRole::Focus);
                    rounded(
                        self.image,
                        chip,
                        8. * s,
                        if selected {
                            mix(fill, self.palette.surface, 0.67)
                        } else {
                            self.appearance.role(ColorRole::AccentSurface)
                        },
                        1.,
                    );
                    emblem(
                        self.image,
                        (chip.x + 7. * s, chip.y + 4. * s),
                        0.75 * s,
                        Emblem::Check,
                        focus,
                    );
                    self.label(
                        "Current",
                        ((chip.x - self.left) / s + 29., center + 5.),
                        13.,
                        51. * s,
                        focus,
                        Weight::SEMIBOLD,
                    );
                }
                hits.push((control.id, rect));
                continue;
            }
            if !panel.dialog && treatment == Treatment::Choice {
                let destination = if let Some(value) = label.strip_prefix("Desktop: ") {
                    Some((Emblem::Display, format!("Desktop · {value}")))
                } else if label.starts_with("Choose account") {
                    Some((Emblem::Accounts, "Choose account".into()))
                } else if label.starts_with("Change account") {
                    Some((Emblem::Accounts, "Change account".into()))
                } else {
                    None
                };
                if let Some((icon, value)) = destination {
                    let offset = (rect.x - self.left) / s;
                    let available = width - 62. * s;
                    emblem(
                        self.image,
                        (rect.x + 12. * s, rect.y + 17. * s),
                        0.75 * s,
                        icon,
                        self.palette.muted,
                    );
                    self.label(
                        &value,
                        (offset + 37., 31.),
                        14.,
                        available,
                        self.palette.text,
                        Weight::SEMIBOLD,
                    );
                    emblem(
                        self.image,
                        (rect.x + width - 27. * s, rect.y + 18. * s),
                        s * 2. / 3.,
                        Emblem::Chevron,
                        self.palette.muted,
                    );
                    hits.push((control.id, rect));
                    continue;
                }
            }
            let leading = treatment != Treatment::Action;
            let weight = if primary {
                Weight::SEMIBOLD
            } else {
                Weight::MEDIUM
            };
            let identity = panel.dialog
                && treatment == Treatment::Choice
                && label.starts_with("Account: ")
                && self.avatar(
                    Rect {
                        x: rect.x + 12. * s,
                        y: rect.y + (rect.height - 26. * s) / 2.,
                        width: 26. * s,
                        height: 26. * s,
                    },
                    8. * s,
                );
            let x = match treatment {
                Treatment::Choice if identity => rect.x + 47. * s,
                Treatment::Choice => rect.x + 13. * s,
                Treatment::Utility => rect.x + if back { 10. * s } else { 0. },
                Treatment::Action => rect.x + width / 2.,
            };
            self.view.text_renderer.borrow_mut().draw_styled(
                self.image,
                label,
                (x, rect.y + rect.height / 2. + size * 0.36 * s),
                size * s,
                rect.width
                    - if identity {
                        85. * s
                    } else if leading && !back {
                        46. * s
                    } else {
                        26. * s
                    },
                ink,
                None,
                if leading {
                    Style::Leading(weight)
                } else {
                    Style::Centered(weight)
                },
            );
            if leading && !label.starts_with("← ") {
                let choice = treatment == Treatment::Choice;
                let offset = if choice {
                    width - 35. * s
                } else {
                    (self.view.text_width(label, size * s) + 10. * s).min(width - 28. * s)
                };
                let icon = if choice {
                    panel
                        .choices
                        .iter()
                        .find(|(id, _)| *id == control.id)
                        .map_or(Emblem::Cycle, |(_, icon)| *icon)
                } else {
                    Emblem::Chevron
                };
                emblem(
                    self.image,
                    (
                        x + offset - if choice { 6. * s } else { 0. },
                        rect.y + rect.height / 2. - 8. * s,
                    ),
                    s * 2. / 3.,
                    icon,
                    ink,
                );
            }
            if back && controls.len() == 1 {
                self.chooser_shortcuts();
            }
            hits.push((control.id, rect));
        }
    }

    fn chooser_shortcuts(&mut self) {
        let s = self.scale;
        let label = "↑ ↓ to move · Enter to select";
        let width = self.view.text_width(label, 13.5 * s);
        self.label(
            label,
            ((self.width - width) / s - 8., 23.),
            13.5,
            width + 2. * s,
            self.palette.muted,
            Weight::NORMAL,
        );
    }
}

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

fn request_height(lines: &[String], subject: bool, metadata: usize, key_request: bool) -> f32 {
    let heading = if key_request {
        46.
    } else if !lines.is_empty() || subject {
        69.
    } else {
        52.
    };
    let line_count = if key_request && fingerprint_groups(lines).is_some() {
        2
    } else {
        lines.len()
    };
    heading + line_count as f32 * 21. + if subject { 23. } else { 0. } + metadata as f32 * 21.
}
