use super::super::{Capabilities, Media, Printer, PrinterState, catalog};
use super::*;
use std::ffi::CStr;

pub(super) fn string(pointer: *const c_char) -> anyhow::Result<Option<String>> {
    if pointer.is_null() {
        return Ok(None);
    }
    // SAFETY: callers pass CUPS-owned NUL-terminated strings while retaining
    // their connection, destination array or capability object.
    let value = unsafe { CStr::from_ptr(pointer) };
    anyhow::ensure!(
        value.to_bytes().len() <= 4096,
        "Printer metadata string exceeds limits"
    );
    Ok(Some(value.to_str()?.to_owned()))
}
fn label(value: String) -> String {
    let mut result = String::new();
    for character in value.chars().filter(|c| !c.is_control()) {
        if result.len() + character.len_utf8() > 256 {
            break;
        }
        result.push(character);
    }
    result
}
fn option(session: &Session, row: &RawDestination, key: &CStr) -> anyhow::Result<Option<String>> {
    anyhow::ensure!(
        (0..=4096).contains(&row.num_options) && (row.num_options == 0 || !row.options.is_null()),
        "Invalid printer option metadata"
    );
    // SAFETY: row points into a retained CUPS array with its valid option count.
    string(unsafe { (session.api.option)(key.as_ptr(), row.num_options, row.options) })
}

pub(super) fn metadata(selected: &Selected<'_>, key: &CStr) -> anyhow::Result<Option<String>> {
    // SAFETY: Selected retains its single public destination record.
    option(selected.session, unsafe { selected.raw.as_ref() }, key)
}
pub(super) fn authentication(
    selected: &Selected<'_>,
) -> anyhow::Result<crate::print::authentication::Requirement> {
    crate::print::authentication::Requirement::parse(
        metadata(selected, c"auth-info-required")?.as_deref(),
    )
}
pub(super) fn versions(selected: &Selected<'_>) -> anyhow::Result<Vec<String>> {
    Attributes { selected }.strings(c"ipp-versions", false)
}

struct Destinations<'a> {
    session: &'a Session,
    raw: *mut RawDestination,
    count: c_int,
}
impl Drop for Destinations<'_> {
    fn drop(&mut self) {
        if !self.raw.is_null() && self.count >= 0 {
            // SAFETY: the pointer/count returned by this session's cupsGetDests2.
            unsafe {
                (self.session.api.free_destinations)(self.count, self.raw);
            }
        }
    }
}

pub(in crate::print::worker) fn printers(cancel: &Cancellation) -> anyhow::Result<Vec<Printer>> {
    cancel.check()?;
    let session = Session::open()?;
    let mut raw = ptr::null_mut();
    // SAFETY: a live connection and initialized out-pointer are supplied; the
    // returned counted public array stays allocated through its guard.
    let count = unsafe { (session.api.destinations)(session.http.as_ptr(), &mut raw) };
    let destinations = Destinations {
        session: &session,
        raw,
        count,
    };
    anyhow::ensure!(
        count >= 0 && count as usize <= catalog::MAX_PRINTERS,
        "Printer listing exceeds limits"
    );
    if count == 0 {
        // SAFETY: the API returns the current thread's last CUPS operation status.
        let status = unsafe { (session.api.last_error)() };
        // CUPS_GET_PRINTERS reports client-error-not-found when the scheduler
        // has no queues. This listing result is not a job acknowledgement.
        anyhow::ensure!(
            succeeded(status) || status == 0x0406,
            "Printer listing unavailable"
        );
        return Ok(Vec::new());
    }
    anyhow::ensure!(!raw.is_null(), "Printer listing unavailable");
    // SAFETY: the validated positive count describes the retained public array.
    let rows = unsafe { std::slice::from_raw_parts(destinations.raw, count as usize) };
    let mut result = Vec::new();
    for row in rows {
        cancel.check()?;
        let destination = Destination {
            queue: string(row.name)?.ok_or_else(|| anyhow::anyhow!("Printer name unavailable"))?,
            instance: string(row.instance)?,
        };
        destination.validate()?;
        let name = label(
            option(&session, row, c"printer-info")?
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| destination.queue.clone()),
        );
        let location = label(option(&session, row, c"printer-location")?.unwrap_or_default());
        let state = match option(&session, row, c"printer-state")?.as_deref() {
            Some("3") => PrinterState::Idle,
            Some("4") => PrinterState::Processing,
            Some("5") => PrinterState::Stopped,
            _ => PrinterState::Unknown,
        };
        let accepting =
            option(&session, row, c"printer-is-accepting-jobs")?.as_deref() == Some("true");
        result.push(Printer {
            destination,
            label: name,
            location,
            is_default: row.is_default != 0,
            state,
            accepting,
        });
    }
    catalog::printers(&result)?;
    cancel.check()?;
    Ok(result)
}

struct Attributes<'a> {
    selected: &'a Selected<'a>,
}
impl Attributes<'_> {
    fn values(
        &self,
        field: super::super::SettingField,
        default: bool,
    ) -> anyhow::Result<Vec<String>> {
        use super::super::SettingField;
        if matches!(field, SettingField::Source | SettingField::Type) {
            return self.strings(field.ipp(), default);
        }
        let attribute = self.attribute(field.ipp(), default);
        if attribute.is_null() {
            return Ok(Vec::new());
        }
        let api = &self.selected.session.api;
        // SAFETY: borrowed attribute; validate tag and count before access.
        let (tag, count) = unsafe {
            (
                (api.attribute_tag)(attribute),
                (api.attribute_count)(attribute),
            )
        };
        let expected = if matches!(field, SettingField::Quality) {
            0x23
        } else {
            0x32
        };
        anyhow::ensure!(
            tag == expected && (0..=catalog::MAX_CHOICES as c_int).contains(&count),
            "Invalid printer control attribute"
        );
        let mut values: Vec<String> = Vec::new();
        for index in 0..count {
            let value = if matches!(field, SettingField::Quality) {
                // SAFETY: checked enum tag and in-range index.
                unsafe { (api.attribute_integer)(attribute, index) }.to_string()
            } else {
                let mut y = 0;
                let mut units = 0;
                // SAFETY: checked resolution tag/index; initialized outputs.
                let x = unsafe { (api.attribute_resolution)(attribute, index, &mut y, &mut units) };
                let unit = match units {
                    3 => "dpi",
                    4 => "dpcm",
                    _ => anyhow::bail!("Invalid printer resolution units"),
                };
                if x == y {
                    format!("{x}{unit}")
                } else {
                    format!("{x}x{y}{unit}")
                }
            };
            field.validate(&value)?;
            if !values.iter().any(|v| field.equivalent(v, &value)) {
                values.push(value);
            }
        }
        Ok(values)
    }
    fn controls(&self, driver: Option<&ppd::Driver<'_>>) -> anyhow::Result<super::super::Controls> {
        use super::super::{Control, Controls, SettingChoice, SettingField};
        let mut controls = Controls::default();
        for field in SettingField::ALL {
            let mut choices = Vec::new();
            let mut aliases = Vec::new();
            for value in self.values(field, false)? {
                field.validate(&value)?;
                let mapped = driver
                    .map(|d| d.mapped(field, &value))
                    .transpose()?
                    .flatten();
                aliases.push((value.clone(), mapped.as_ref().map(|m| m.value.clone())));
                choices.push(SettingChoice {
                    label: mapped.map_or_else(|| field.value_label(&value), |m| m.label),
                    value,
                });
            }
            let saved = self
                .preference(field.ipp())?
                .map(|value| {
                    choices
                        .iter()
                        .find(|c| field.equivalent(&c.value, &value))
                        .map(|c| c.value.clone())
                        .ok_or_else(|| anyhow::anyhow!("Saved printer control is unavailable"))
                })
                .transpose()?;
            let saved_ppd = self.preference(field.ppd())?;
            let resolve = |value: &str| -> anyhow::Result<String> {
                let matching = aliases
                    .iter()
                    .filter(|(_, alias)| {
                        alias.as_deref().is_some_and(|a| {
                            a.eq_ignore_ascii_case(value) || field.equivalent(a, value)
                        })
                    })
                    .map(|(value, _)| value)
                    .collect::<Vec<_>>();
                anyhow::ensure!(
                    matching.len() == 1,
                    "Saved driver control is unavailable or ambiguous"
                );
                Ok(matching[0].clone())
            };
            let alias = saved_ppd.as_deref().map(resolve).transpose()?;
            anyhow::ensure!(
                saved
                    .as_ref()
                    .zip(alias.as_ref())
                    .is_none_or(|(a, b)| a == b),
                "Conflicting saved printer controls"
            );
            let default = saved.or(alias).or(self
                .values(field, true)?
                .into_iter()
                .find(|v| choices.iter().any(|c| &c.value == v)));
            let default = if default.is_none() {
                driver
                    .map(|d| d.default(field))
                    .transpose()?
                    .flatten()
                    .and_then(|v| resolve(&v).ok())
            } else {
                default
            };
            controls.set(field, Control { choices, default });
        }
        controls.validate()?;
        Ok(controls)
    }
    fn preference(&self, key: &CStr) -> anyhow::Result<Option<String>> {
        // SAFETY: Selected retains the single named destination until this
        // capability query ends, including its CUPS-owned option array.
        option(
            self.selected.session,
            unsafe { self.selected.raw.as_ref() },
            key,
        )
    }
    fn attribute(&self, key: &CStr, default: bool) -> *mut c_void {
        let session = self.selected.session;
        let function = if default {
            session.api.find_default
        } else {
            session.api.find_supported
        };
        // SAFETY: all objects and the static attribute name remain live; the
        // returned attribute is borrowed from the retained capability object.
        unsafe {
            function(
                session.http.as_ptr(),
                self.selected.raw.as_ptr(),
                self.selected.info.expect("capabilities").as_ptr(),
                key.as_ptr(),
            )
        }
    }
    fn strings(&self, key: &CStr, default: bool) -> anyhow::Result<Vec<String>> {
        let attribute = self.attribute(key, default);
        if attribute.is_null() {
            return Ok(Vec::new());
        }
        let api = &self.selected.session.api;
        // SAFETY: attribute belongs to the retained capability object.
        let (tag, count) = unsafe {
            (
                (api.attribute_tag)(attribute),
                (api.attribute_count)(attribute),
            )
        };
        anyhow::ensure!(
            (0..=catalog::MAX_CHOICES as c_int).contains(&count),
            "Too many printer option values"
        );
        if ![0x41, 0x42, 0x44, 0x49].contains(&tag) {
            return Ok(Vec::new());
        }
        let mut values = Vec::new();
        for index in 0..count {
            // SAFETY: a textual attribute and in-range element index are used;
            // no optional language out-pointer is requested.
            if let Some(value) =
                string(unsafe { (api.attribute_string)(attribute, index, ptr::null_mut()) })?
                && !value.is_empty()
            {
                anyhow::ensure!(
                    value.len() <= 127 && !value.chars().any(char::is_control),
                    "Invalid printer option value"
                );
                if !values.contains(&value) {
                    values.push(value);
                }
            }
        }
        Ok(values)
    }
    fn copies(&self) -> Option<(u32, u32)> {
        let attribute = self.attribute(c"copies", false);
        if attribute.is_null() {
            return None;
        }
        let api = &self.selected.session.api;
        // SAFETY: attribute remains allocated, and range access happens only
        // for a single range-valued attribute.
        unsafe {
            if (api.attribute_tag)(attribute) != 0x33 || (api.attribute_count)(attribute) != 1 {
                return None;
            }
            let mut maximum = 0;
            let minimum = (api.attribute_range)(attribute, 0, &mut maximum);
            (minimum > 0 && minimum <= maximum && minimum <= 9999)
                .then_some((minimum as u32, maximum.min(9999) as u32))
        }
    }
}

fn sides(value: &str) -> Option<Sides> {
    match value {
        "one-sided" => Some(Sides::OneSided),
        "two-sided-long-edge" => Some(Sides::LongEdge),
        "two-sided-short-edge" => Some(Sides::ShortEdge),
        _ => None,
    }
}
fn color(value: &str) -> Option<bool> {
    match value {
        "color" => Some(true),
        "monochrome" => Some(false),
        _ => None,
    }
}

pub(in crate::print::worker) fn capabilities(
    destination: Destination,
    preferences: Option<&super::super::Preferences>,
    cancel: &Cancellation,
) -> anyhow::Result<Capabilities> {
    if let Some(preferences) = preferences {
        preferences.validate()?;
    }
    cancel.check()?;
    let session = Session::open()?;
    let selected = session.destination(&destination, true)?;
    let attributes = Attributes {
        selected: &selected,
    };
    let driver = ppd::Driver::fetch(&selected)?;
    let controls = attributes.controls(driver.as_ref())?;
    let advertised_media = attributes.strings(c"media", false)?;
    let custom_media = advertised_media
        .iter()
        .any(|id| id.starts_with("custom_min_"))
        && advertised_media
            .iter()
            .any(|id| id.starts_with("custom_max_"));
    let mut media = Vec::new();
    for id in advertised_media {
        cancel.check()?;
        // Ranges are not fixed sheets; the custom query resolves concrete sizes.
        if id.starts_with("custom_min_") || id.starts_with("custom_max_") {
            continue;
        }
        let name = CString::new(id.as_str())?;
        let mut size = RawMedia {
            media: [0; 128],
            width: 0,
            height: 0,
            bottom: 0,
            left: 0,
            right: 0,
            top: 0,
        };
        // SAFETY: initialized public cups_size_t layout, live objects and string;
        // the call fills this one output without transferring its ownership.
        let found = unsafe {
            (session.api.media_by_name)(
                session.http.as_ptr(),
                selected.raw.as_ptr(),
                selected.info.expect("capabilities").as_ptr(),
                name.as_ptr(),
                0,
                &mut size,
            )
        };
        if found != 0 {
            let value = Media {
                id,
                width_mm: f64::from(size.width) / 100.,
                height_mm: f64::from(size.height) / 100.,
                top_mm: f64::from(size.top) / 100.,
                bottom_mm: f64::from(size.bottom) / 100.,
                left_mm: f64::from(size.left) / 100.,
                right_mm: f64::from(size.right) / 100.,
            };
            value.page_setup().validate()?;
            media.push(value);
        }
    }
    let mut default_media = attributes
        .preference(c"media")?
        .filter(|id| media.iter().any(|media| &media.id == id))
        .or(attributes
            .strings(c"media", true)?
            .into_iter()
            .find(|id| media.iter().any(|media| &media.id == id)));
    let supported_sides = attributes
        .strings(c"sides", false)?
        .iter()
        .filter_map(|value| sides(value))
        .collect::<Vec<_>>();
    let default_sides = attributes
        .preference(c"sides")?
        .as_deref()
        .and_then(sides)
        .filter(|value| supported_sides.contains(value))
        .or(attributes
            .strings(c"sides", true)?
            .iter()
            .find_map(|value| sides(value))
            .filter(|value| supported_sides.contains(value)));
    let colors = attributes
        .strings(c"print-color-mode", false)?
        .iter()
        .filter_map(|value| color(value))
        .collect::<Vec<_>>();
    let default_color = attributes
        .preference(c"print-color-mode")?
        .as_deref()
        .and_then(color)
        .filter(|value| colors.contains(value))
        .or(attributes
            .strings(c"print-color-mode", true)?
            .iter()
            .find_map(|value| color(value))
            .filter(|value| colors.contains(value)));
    let preference = attributes
        .preference(c"media")?
        .or(attributes.preference(c"PageSize")?)
        .or_else(|| attributes.strings(c"media", true).ok()?.into_iter().next());
    let default_custom_media = if let Some(name) =
        preference.filter(|name| super::media::ppd_custom_name(name) || name.starts_with("custom_"))
    {
        anyhow::ensure!(custom_media, "Saved custom paper is unavailable");
        let value = super::media::preference(&selected, &name, default_sides)?;
        default_media = None;
        Some(value)
    } else {
        None
    };
    let mut result = Capabilities {
        destination,
        copies: attributes.copies(),
        media,
        default_media,
        custom_media,
        default_custom_media,
        sides: supported_sides,
        default_sides,
        colors,
        default_color,
        formats: attributes.strings(c"document-format", false)?,
        controls: Box::new(controls),
        preferences: None,
        authentication: authentication(&selected)?,
    };
    result.validate()?;
    if let Some(preferences) = preferences {
        let mut aliases = Vec::new();
        if let Some(driver) = &driver {
            for field in super::super::SettingField::ALL {
                for choice in &result.controls.get(field).choices {
                    cancel.check()?;
                    if let Some(mapped) = driver.mapped(field, &choice.value)? {
                        aliases.push((field, choice.value.clone(), mapped.value));
                    }
                }
            }
        }
        let mut paper_aliases = Vec::new();
        for name in preferences.paper_names() {
            let name_c = CString::new(name)?;
            // SAFETY: bounded NUL-terminated name and retained process library.
            // Copy borrowed thread-local strings before another CUPS call.
            if let Some(raw) = unsafe { (session.api.media_for_ppd)(name_c.as_ptr()).as_ref() }
                && let Some(pwg) = string(raw.pwg)?
            {
                paper_aliases.push((name.to_owned(), pwg));
            }
        }
        result.preferences = Some(Box::new(preferences.resolve(
            &result,
            &aliases,
            &paper_aliases,
        )));
        result.validate()?;
    }
    cancel.check()?;
    Ok(result)
}
