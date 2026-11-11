use super::*;
use crate::print::{Orientation, PageRange, Pages, worker::Sides};

pub(super) struct Draft {
    pub layout: Layout,
    pub setup: PageSetup,
    pub device: JobOptions,
    pub copies: String,
    pub scale: String,
    pub ranges: String,
    pub format: Option<Format>,
    pub path: Option<Vec<u8>>,
    pub reviewed: Option<Arc<crate::print::file_target::Reviewed>>,
    pub geometry: [String; 6],
    pub custom: bool,
    pub custom_dimensions: [String; 2],
}
#[derive(Clone, Debug)]
pub(super) enum Edit {
    Copies(String),
    Scale(String),
    Ranges(String),
    Pages(Choice<Pages>),
    Orientation(Choice<Orientation>),
    Collate(bool),
    Reverse(bool),
    PageSet(Choice<String>),
    NumberUp(Choice<u32>),
    NumberLayout(Choice<String>),
    Media(Choice<Option<String>>),
    Custom(bool),
    CustomDimension(usize, String),
    Sides(Choice<Option<Sides>>),
    Color(Choice<Option<bool>>),
    Setting(crate::print::worker::SettingField, Choice<Option<String>>),
    Format(Choice<Format>),
    Paper(Choice<(u32, u32)>),
    Geometry(usize, String),
}
impl Draft {
    pub fn new(request: &Request) -> Self {
        let layout = request.prepare.settings.layout.clone();
        let ranges = PageRange::cups(&layout.ranges).unwrap_or_default();
        let mut setup = request.prepare.page_setup.clone();
        // A file has no device default. Present an editable named paper preset;
        // retain an application's explicit valid dimensions where supplied.
        if setup.width.is_none() {
            setup.width = Some(210.);
            setup.height = Some(297.);
            setup.name = Some("iso_a4".into());
        }
        let geometry = [
            setup.width,
            setup.height,
            setup.top,
            setup.bottom,
            setup.left,
            setup.right,
        ]
        .map(|value| value.unwrap_or(0.).to_string());
        Self {
            copies: layout.copies.to_string(),
            scale: layout.scale.to_string(),
            ranges,
            layout,
            setup,
            geometry,
            device: JobOptions::default(),
            custom: false,
            custom_dimensions: ["210".into(), "297".into()],
            format: request
                .formats()
                .iter()
                .copied()
                .find(|format| {
                    Some(format.name()) == request.prepare.settings.advisory("output-file-format")
                })
                .or_else(|| request.formats().first().copied()),
            path: None,
            reviewed: None,
        }
    }
    pub fn install(&mut self, caps: &Capabilities) {
        self.device.sides = caps.default_sides;
        self.device.color = caps.default_color;
        self.device.settings = caps.controls.defaults();
        self.default_paper(caps);
    }
    pub fn restore(&mut self, selection: &crate::print::worker::PreferenceSelection) {
        self.device = selection.options.clone();
        self.custom = selection.custom_size.is_some();
        if let Some(size) = selection.custom_size {
            self.custom_dimensions = [f64::from(size.width) / 100., f64::from(size.height) / 100.]
                .map(|v| v.to_string());
        }
    }
    pub fn default_paper(&mut self, caps: &Capabilities) {
        self.device.media = caps.default_media.clone();
        self.custom = caps.default_custom_media.is_some();
        // A changed side setting needs its own printable-area lookup. Keep the
        // default dimensions visible while requiring a fresh confirmation.
        self.device.custom_media = caps
            .default_custom_media
            .clone()
            .filter(|_| self.device.sides == caps.default_sides);
        if let Some(media) = caps.default_custom_media.as_ref().or_else(|| {
            caps.media
                .iter()
                .find(|media| Some(&media.id) == caps.default_media.as_ref())
        }) {
            self.custom_dimensions = [media.width_mm.to_string(), media.height_mm.to_string()];
        }
    }
    pub fn fixed_paper(&mut self, caps: &Capabilities) {
        if let Some(id) = caps
            .default_media
            .as_ref()
            .or_else(|| caps.media.first().map(|media| &media.id))
        {
            self.custom = false;
            self.device.media = Some(id.clone());
            self.device.custom_media = None;
        } else {
            self.default_paper(caps);
        }
    }
    pub fn edit(&mut self, edit: Edit) {
        match edit {
            Edit::Copies(value) => self.copies = value,
            Edit::Scale(value) => self.scale = value,
            Edit::Ranges(value) => self.ranges = value,
            Edit::Pages(choice) => self.layout.pages = choice.value,
            Edit::Orientation(choice) => self.layout.orientation = choice.value,
            Edit::Collate(value) => self.layout.collate = value,
            Edit::Reverse(value) => self.layout.reverse = value,
            Edit::PageSet(choice) => self.layout.page_set = choice.value,
            Edit::NumberUp(choice) => self.layout.number_up = choice.value,
            Edit::NumberLayout(choice) => self.layout.number_up_layout = choice.value,
            Edit::Media(choice) => {
                self.device.media = choice.value;
                self.device.custom_media = None;
            }
            Edit::Custom(value) => {
                self.custom = value;
                self.device.custom_media = None;
                self.device.media = None;
            }
            Edit::CustomDimension(index, value) => {
                if let Some(field) = self.custom_dimensions.get_mut(index) {
                    *field = value;
                    self.device.custom_media = None;
                }
            }
            Edit::Sides(choice) => {
                self.device.sides = choice.value;
                self.device.custom_media = None;
            }
            Edit::Color(choice) => self.device.color = choice.value,
            Edit::Setting(field, choice) => self.device.settings.set(field, choice.value),
            Edit::Format(choice) => {
                self.format = Some(choice.value);
                self.path = None;
                self.reviewed = None;
            }
            Edit::Paper(choice) => {
                self.geometry[0] = (f64::from(choice.value.0) / 10.).to_string();
                self.geometry[1] = (f64::from(choice.value.1) / 10.).to_string();
                self.setup.name = None;
                self.setup.ppd_name = None;
                self.setup.display_name = None;
            }
            Edit::Geometry(index, value) => {
                if let Some(field) = self.geometry.get_mut(index) {
                    *field = value;
                }
                self.setup.name = None;
                self.setup.ppd_name = None;
                self.setup.display_name = None;
            }
        }
    }
    pub fn values(
        &self,
        request: &Request,
        file: bool,
    ) -> anyhow::Result<(Layout, PageSetup, JobOptions)> {
        let mut layout = self.layout.clone();
        anyhow::ensure!(
            self.copies.len() <= 4 && self.copies.bytes().all(|b| b.is_ascii_digit()),
            "Invalid copies"
        );
        layout.copies = self.copies.parse()?;
        anyhow::ensure!(self.scale.len() <= 16, "Invalid scale");
        layout.scale = self.scale.parse()?;
        layout.ranges = if layout.pages == Pages::Ranges {
            ranges(&self.ranges)?
        } else {
            Vec::new()
        };
        layout.validate(request.prepare.page_choices)?;
        let mut setup = self.setup.clone();
        if file {
            let mut values = [0.; 6];
            for (value, field) in values.iter_mut().zip(&self.geometry) {
                anyhow::ensure!(field.len() <= 16, "Invalid page geometry");
                *value = field.parse()?;
            }
            [
                setup.width,
                setup.height,
                setup.top,
                setup.bottom,
                setup.left,
                setup.right,
            ] = values.map(Some);
            // Toolkit consumers require a paper identity even for custom sizes.
            // Retire any old PPD alias when explicit file dimensions are used.
            setup.ppd_name = None;
            setup.name = Some("canopa-custom".into());
            setup.display_name = Some(format!("{} × {} mm", values[0], values[1]));
        }
        setup.orientation = Some(layout.orientation);
        setup.validate()?;
        let mut device = self.device.clone();
        device.copies = layout.copies;
        Ok((layout, setup, device))
    }
}

/// UI ranges are one-based; only this boundary converts to the wire's zero base.
fn ranges(text: &str) -> anyhow::Result<Vec<PageRange>> {
    anyhow::ensure!(text.len() <= 4096, "Page ranges exceed limits");
    let ranges = PageRange::parse(text)?;
    ranges
        .into_iter()
        .map(|range| {
            Ok(PageRange {
                first: range
                    .first
                    .checked_sub(1)
                    .ok_or_else(|| anyhow::anyhow!("Pages start at 1"))?,
                last: range
                    .last
                    .checked_sub(1)
                    .ok_or_else(|| anyhow::anyhow!("Pages start at 1"))?,
            })
        })
        .collect()
}

pub(super) fn file_stem(title: &str) -> String {
    let value: String = title
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '/' | '\\'))
        .take(80)
        .collect();
    let value = value.trim().trim_matches('.');
    if value.is_empty() {
        fl!("portal-print-document")
    } else {
        value.into()
    }
}
