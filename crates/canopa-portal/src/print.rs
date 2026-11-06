//! Bounded Print request data. Printer/file authority is kept out of caller hints.
//!
//! This is the contract foundation; no Print interface is advertised yet.
pub mod authority;
#[cfg(feature = "desktop")]
pub mod file_target;
#[cfg(feature = "desktop")]
pub mod jobs_review;
#[cfg(feature = "desktop")]
pub mod live_jobs;
#[cfg(feature = "desktop")]
pub(crate) mod printer_jobs;
#[cfg(feature = "desktop")]
mod recovery_host;
#[cfg(feature = "desktop")]
pub use recovery_host::RecoveryHost;
#[cfg(feature = "desktop")]
pub mod presentation;
#[cfg(feature = "desktop")]
mod production;
#[cfg(feature = "desktop")]
pub mod render;
#[cfg(feature = "desktop")]
pub use production::Production;
#[cfg(feature = "desktop")]
mod service;
#[cfg(feature = "desktop")]
pub use service::{Backend, Delivery, Print, PrinterJobs, Rendering};
#[cfg(feature = "desktop")]
pub(crate) mod authentication;
mod settings;
pub mod tokens;
#[cfg(feature = "desktop")]
pub mod worker;
pub use settings::{Layout, Orientation, PageRange, PageSetup, Pages, SettingsHints};

use crate::requests::{Options, option};
use serde::{Deserialize, Serialize};
use zbus::fdo;

pub const MAX_REQUEST_BYTES: u32 = 128 * 1024;
const MAX_FIELDS: usize = 64;
const MAX_FIELD_BYTES: usize = 4096;
const MAX_TEXT_BYTES: usize = 64 * 1024;

fn invalid(message: &str) -> fdo::Error {
    fdo::Error::InvalidArgs(message.into())
}
fn dictionary(values: &Options) -> fdo::Result<()> {
    if values.len() > MAX_FIELDS
        || values
            .keys()
            .any(|key| key.len() > 128 || key.is_empty() || key.chars().any(char::is_control))
    {
        return Err(invalid("Print dictionary exceeds limits"));
    }
    Ok(())
}
fn text(value: &str, budget: &mut usize) -> fdo::Result<()> {
    *budget = budget.saturating_add(value.len());
    if value.len() > MAX_FIELD_BYTES || value.contains('\0') || *budget > MAX_TEXT_BYTES {
        return Err(invalid("Print text exceeds limits"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Pdf,
    Ps,
    Svg,
}
impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Ps => "ps",
            Self::Svg => "svg",
        }
    }
    pub fn mime(self) -> &'static str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Ps => "application/postscript",
            Self::Svg => "image/svg+xml",
        }
    }
    fn parse(value: &str) -> fdo::Result<Self> {
        match value {
            "pdf" => Ok(Self::Pdf),
            "ps" => Ok(Self::Ps),
            "svg" => Ok(Self::Svg),
            _ => Err(invalid("Unsupported print output format")),
        }
    }
}
fn formats(options: &mut Options) -> fdo::Result<Vec<Format>> {
    let Some(values): Option<Vec<String>> = option(options, "supported_output_file_formats")?
    else {
        // The published contract assumes all three when this option is absent.
        return Ok(vec![Format::Pdf, Format::Ps, Format::Svg]);
    };
    if values.len() > 3 {
        return Err(invalid("Too many print output formats"));
    }
    let mut parsed = Vec::new();
    for value in values {
        let format = Format::parse(&value)?;
        if !parsed.contains(&format) {
            parsed.push(format);
        }
    }
    // An explicitly empty list disables print-to-file, not printer selection.
    Ok(parsed)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub app_id: String,
    pub parent_window: String,
    pub title: String,
    pub modal: bool,
}
impl Context {
    fn parse(
        app_id: String,
        parent_window: String,
        title: String,
        options: &mut Options,
    ) -> fdo::Result<Self> {
        let mut budget = 0;
        for value in [&app_id, &parent_window, &title] {
            text(value, &mut budget)?;
        }
        if app_id.len() > 256 {
            return Err(invalid("Print application ID exceeds limits"));
        }
        Ok(Self {
            app_id,
            parent_window,
            title,
            modal: option(options, "modal")?.unwrap_or(true),
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageChoices {
    pub current: bool,
    pub selection: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareRequest {
    pub context: Context,
    pub accept_label: String,
    pub formats: Vec<Format>,
    pub page_choices: PageChoices,
    pub settings: SettingsHints,
    pub page_setup: PageSetup,
}
impl PrepareRequest {
    pub fn parse(
        app_id: String,
        parent_window: String,
        title: String,
        settings: Options,
        page_setup: Options,
        mut options: Options,
    ) -> fdo::Result<Self> {
        dictionary(&options)?;
        let context = Context::parse(app_id, parent_window, title, &mut options)?;
        let accept_label: String = option(&mut options, "accept_label")?
            .filter(|value: &String| !value.is_empty())
            .unwrap_or_else(|| "Print".into());
        text(&accept_label, &mut 0)?;
        let page_choices = PageChoices {
            current: option(&mut options, "has_current_page")?.unwrap_or(false),
            selection: option(&mut options, "has_selected_pages")?.unwrap_or(false),
        };
        let mut settings = SettingsHints::parse(settings)?;
        // Initial settings are preferences. Old clients can retain a page mode
        // they cannot currently offer; present All instead of granting that mode.
        if matches!(settings.layout.pages, Pages::Current) && !page_choices.current
            || matches!(settings.layout.pages, Pages::Selection) && !page_choices.selection
        {
            settings.layout.pages = Pages::All;
        }
        Ok(Self {
            context,
            accept_label,
            formats: formats(&mut options)?,
            page_choices,
            settings,
            page_setup: PageSetup::parse(page_setup)?,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrintOptions {
    pub context: Context,
    pub token: Option<u32>,
    pub formats: Vec<Format>,
}
impl PrintOptions {
    pub fn parse(
        app_id: String,
        parent_window: String,
        title: String,
        mut options: Options,
    ) -> fdo::Result<Self> {
        dictionary(&options)?;
        Ok(Self {
            context: Context::parse(app_id, parent_window, title, &mut options)?,
            token: option(&mut options, "token")?,
            formats: formats(&mut options)?,
        })
    }
}

#[cfg(test)]
mod test_bus;
#[cfg(test)]
mod tests;
