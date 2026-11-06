//! Compatibility with legacy queues whose destination API omits constraints.
//! Only the public read/mark API is used; no PPD code or filters are executed.
use super::*;
use crate::print::worker::{SettingChoice, SettingField};
use std::{
    io::{Read, Write},
    os::fd::AsRawFd,
};

const MAX_BYTES: usize = 1024 * 1024;
type RawPpd = *mut c_void;

// Public cups/ppd.h records. The ppd_file_t itself remains opaque.
#[repr(C)]
struct Choice {
    marked: c_char,
    name: [c_char; 41],
    text: [c_char; 81],
    code: *mut c_char,
    option: *mut c_void,
}
#[repr(C)]
struct OptionRecord {
    conflicted: c_char,
    keyword: [c_char; 41],
    default: [c_char; 41],
    text: [c_char; 81],
    ui: c_int,
    section: c_int,
    order: f32,
    count: c_int,
    choices: *mut Choice,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Size {
    marked: c_int,
    name: [c_char; 41],
    width: f32,
    length: f32,
    left: f32,
    bottom: f32,
    right: f32,
    top: f32,
}
impl Default for Size {
    fn default() -> Self {
        Self {
            marked: 0,
            name: [0; 41],
            width: 0.,
            length: 0.,
            left: 0.,
            bottom: 0.,
            right: 0.,
            top: 0.,
        }
    }
}
struct Functions {
    open: unsafe extern "C" fn(*const c_char) -> RawPpd,
    close: unsafe extern "C" fn(RawPpd),
    defaults: unsafe extern "C" fn(RawPpd),
    option: unsafe extern "C" fn(RawPpd, *const c_char) -> *mut OptionRecord,
    marked: unsafe extern "C" fn(RawPpd, *const c_char) -> *mut Choice,
    mark_one: unsafe extern "C" fn(RawPpd, *const c_char, *const c_char) -> c_int,
    mark: unsafe extern "C" fn(RawPpd, c_int, *mut OptionValue) -> c_int,
    conflicts: unsafe extern "C" fn(RawPpd) -> c_int,
    limits: unsafe extern "C" fn(RawPpd, *mut Size, *mut Size) -> c_int,
    page_size: unsafe extern "C" fn(RawPpd, *const c_char) -> *const Size,
}
impl Functions {
    fn load(library: &Library) -> anyhow::Result<Self> {
        // SAFETY: public cups/ppd.h signatures, retained library. Required only
        // after an actual PPD is received; IPP-only queues need none of these.
        unsafe {
            Ok(Self {
                open: *library.get(b"ppdOpenFile\0")?,
                close: *library.get(b"ppdClose\0")?,
                defaults: *library.get(b"ppdMarkDefaults\0")?,
                option: *library.get(b"ppdFindOption\0")?,
                marked: *library.get(b"ppdFindMarkedChoice\0")?,
                mark_one: *library.get(b"ppdMarkOption\0")?,
                mark: *library.get(b"cupsMarkOptions\0")?,
                conflicts: *library.get(b"ppdConflicts\0")?,
                limits: *library.get(b"ppdPageSizeLimits\0")?,
                page_size: *library.get(b"ppdPageSize\0")?,
            })
        }
    }
}
pub(super) struct Driver<'a> {
    api: Functions,
    _library: &'a Library,
    file: File,
}
impl<'a> Driver<'a> {
    pub(super) fn fetch(selected: &Selected<'a>) -> anyhow::Result<Option<Self>> {
        let session = selected.session;
        // SAFETY: Selected retains this initialized named destination.
        let queue = discovery::string(unsafe { selected.raw.as_ref().name })?
            .ok_or_else(|| anyhow::anyhow!("Printer name unavailable"))?;
        let resource = CString::new(format!("/printers/{}.ppd", escape(&queue)))?;
        let (read, write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;
        // A bounded reader drains concurrently with cupsGetFd. On overflow it
        // closes the pipe; no unbounded temporary PPD file is ever created.
        let reader = std::thread::Builder::new()
            .name("print-driver".into())
            .spawn(move || {
                let mut bytes = Vec::new();
                File::from(read)
                    .take(MAX_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)?;
                anyhow::ensure!(bytes.len() <= MAX_BYTES, "Printer driver exceeds limits");
                Ok::<_, anyhow::Error>(bytes)
            })?;
        // SAFETY: live connection, escaped resource and owned writable pipe.
        let status = unsafe {
            (session.api.get_fd)(session.http.as_ptr(), resource.as_ptr(), write.as_raw_fd())
        };
        drop(write);
        let bytes = reader
            .join()
            .map_err(|_| anyhow::anyhow!("Printer driver reader failed"))??;
        if status == 404 {
            return Ok(None);
        }
        anyhow::ensure!(
            status == 200 && !bytes.is_empty(),
            "Printer driver unavailable"
        );
        Ok(Some(Self::from_bytes(session.api._library, &bytes)?))
    }
    fn from_bytes(library: &'a Library, bytes: &[u8]) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_BYTES,
            "Invalid printer driver size"
        );
        let api = Functions::load(library)?;
        let mut file = File::from(rustix::fs::memfd_create(
            c"print-driver",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )?);
        file.write_all(bytes)?;
        rustix::fs::fcntl_add_seals(
            &file,
            rustix::fs::SealFlags::SHRINK
                | rustix::fs::SealFlags::GROW
                | rustix::fs::SealFlags::WRITE
                | rustix::fs::SealFlags::SEAL,
        )?;
        let driver = Self {
            api,
            _library: library,
            file,
        };
        driver.open()?;
        Ok(driver)
    }
    fn open(&self) -> anyhow::Result<Marked<'_>> {
        let path = CString::new(format!("/proc/self/fd/{}", self.file.as_raw_fd()))?;
        // SAFETY: retained sealed memfd path; public parser returns an owned
        // opaque object. Every operation starts fresh, including custom state.
        let raw = NonNull::new(unsafe { (self.api.open)(path.as_ptr()) })
            .ok_or_else(|| anyhow::anyhow!("Printer driver could not be read"))?;
        // SAFETY: newly allocated opaque object retained until Marked drops.
        unsafe {
            (self.api.defaults)(raw.as_ptr());
        }
        Ok(Marked {
            api: &self.api,
            raw,
        })
    }
    pub(super) fn default(&self, field: SettingField) -> anyhow::Result<Option<String>> {
        Ok(self.open()?.choice(field)?.map(|choice| choice.value))
    }
    pub(super) fn mapped(
        &self,
        field: SettingField,
        value: &str,
    ) -> anyhow::Result<Option<SettingChoice>> {
        field.validate(value)?;
        let marked = self.open()?;
        let choices = marked.choices(field)?;
        if choices.is_empty() {
            return Ok(None);
        }
        let mut result: Option<SettingChoice> = None;
        // An ignored media token leaves the previous choice unchanged. Probe
        // from both ends so that default-looking aliases cannot silently pass.
        for seed in [choices.first().unwrap(), choices.last().unwrap()] {
            let seed = CString::new(seed.value.as_str())?;
            // SAFETY: keyword and verified driver choice, live opaque object.
            unsafe {
                (self.api.mark_one)(marked.raw.as_ptr(), field.ppd().as_ptr(), seed.as_ptr());
            }
            marked.mark(&[(CString::from(field.mapping()), CString::new(value)?)])?;
            let choice = marked
                .choice(field)?
                .ok_or_else(|| anyhow::anyhow!("Printer control mapping unavailable"))?;
            if let Some(prior) = &result {
                anyhow::ensure!(
                    prior.value == choice.value,
                    "Printer control mapping unavailable"
                );
            }
            result = Some(choice);
        }
        Ok(result)
    }
    pub(super) fn validate(
        &self,
        selected: &Selected<'_>,
        options: &JobOptions,
        values: &[(CString, CString)],
    ) -> anyhow::Result<Vec<(CString, CString)>> {
        let marked = self.open()?;
        // Preserve installable options and saved driver settings before the
        // explicit draft. The destination array remains owned by Selected.
        let row = unsafe { selected.raw.as_ref() };
        anyhow::ensure!(
            (0..=4096).contains(&row.num_options)
                && (row.num_options == 0 || !row.options.is_null()),
            "Invalid saved printer options"
        );
        // SAFETY: bounded initialized CUPS-owned option array and live PPD.
        unsafe {
            (self.api.mark)(marked.raw.as_ptr(), row.num_options, row.options);
        }
        let mut driver_values = Vec::new();
        for field in SettingField::ALL {
            if let Some(value) = options.settings.get(field)
                && let Some(choice) = self.mapped(field, value)?
            {
                driver_values.push((CString::from(field.ppd()), CString::new(choice.value)?));
            }
        }
        let all = values
            .iter()
            .chain(&driver_values)
            .cloned()
            .collect::<Vec<_>>();
        marked.mark(&all)?;
        if let Some(media) = &options.custom_media {
            let size = CustomSize::from_media(media)?;
            if !marked.named_media_matches(media) {
                // Mapping a PWG keyword can select a nearby named sheet or
                // truncate custom dimensions to whole points. Override that
                // mapping with the approved concrete PPD spelling, retaining
                // the original media option and the selected driver controls.
                let paper = (
                    CString::from(c"PageSize"),
                    CString::new(format!(
                        "Custom.{}x{}pt",
                        ppd_points(size.width),
                        ppd_points(size.height)
                    ))?,
                );
                // CUPS 2.x's bulk marker lets media override even an explicit
                // PageSize. Mark this concrete choice after the other options.
                // SAFETY: retained bounded strings and this live opaque PPD;
                // the public marker borrows the strings and resolves no conflicts.
                unsafe {
                    (self.api.mark_one)(marked.raw.as_ptr(), paper.0.as_ptr(), paper.1.as_ptr());
                }
                driver_values.push(paper);
            }
            marked.custom_limits(size)?;
            anyhow::ensure!(
                marked.matches_media(media),
                "Printer driver cannot preserve the approved paper dimensions and margins"
            );
        }
        // No resolver is called: incompatible choices stay editable in Iced.
        anyhow::ensure!(
            unsafe { (self.api.conflicts)(marked.raw.as_ptr()) } == 0,
            "These printer settings cannot be used together"
        );
        Ok(driver_values)
    }
}
struct Marked<'a> {
    api: &'a Functions,
    raw: NonNull<c_void>,
}
impl Marked<'_> {
    fn current_size(&self) -> Option<Size> {
        // SAFETY: the public size record remains borrowed from this live PPD;
        // a null name queries the current marked sheet without changing it.
        // Copy it before any later marking call can mutate the PPD's record.
        unsafe { (self.api.page_size)(self.raw.as_ptr(), ptr::null()).as_ref() }.copied()
    }
    fn matches_media(&self, media: &Media) -> bool {
        self.current_size()
            .is_some_and(|size| geometry_matches(&size, media))
    }
    fn named_media_matches(&self, media: &Media) -> bool {
        self.current_size().is_some_and(|size| {
            bounded(&size.name)
                .is_ok_and(|name| !name.is_empty() && !name.eq_ignore_ascii_case("Custom"))
                && geometry_matches(&size, media)
        })
    }
    fn choices(&self, field: SettingField) -> anyhow::Result<Vec<SettingChoice>> {
        // SAFETY: public record borrowed from the retained opaque PPD.
        let Some(option) =
            (unsafe { (self.api.option)(self.raw.as_ptr(), field.ppd().as_ptr()).as_ref() })
        else {
            return Ok(Vec::new());
        };
        anyhow::ensure!(
            [0, 1].contains(&option.ui)
                && (0..=super::super::catalog::MAX_CHOICES as c_int).contains(&option.count)
                && (option.count == 0 || !option.choices.is_null()),
            "Invalid driver choices"
        );
        if option.count == 0 {
            return Ok(Vec::new());
        }
        // SAFETY: validated count of initialized public choice records.
        unsafe { std::slice::from_raw_parts(option.choices, option.count as usize) }
            .iter()
            .map(choice)
            .collect()
    }
    fn choice(&self, field: SettingField) -> anyhow::Result<Option<SettingChoice>> {
        // SAFETY: borrowed marked choice remains valid while this PPD lives.
        unsafe { (self.api.marked)(self.raw.as_ptr(), field.ppd().as_ptr()).as_ref() }
            .map(choice)
            .transpose()
    }
    fn mark(&self, pairs: &[(CString, CString)]) -> anyhow::Result<()> {
        let mut raw = option_array(pairs);
        // SAFETY: retained strings/array; mark does not take their ownership.
        unsafe {
            (self.api.mark)(self.raw.as_ptr(), raw.len() as c_int, raw.as_mut_ptr());
        }
        Ok(())
    }
    fn custom_limits(&self, size: CustomSize) -> anyhow::Result<()> {
        let mut minimum = Size::default();
        let mut maximum = Size::default();
        // SAFETY: initialized public records filled for this marked PPD.
        let found = unsafe { (self.api.limits)(self.raw.as_ptr(), &mut minimum, &mut maximum) };
        let width = f64::from(size.width) * 72. / 2540.;
        let height = f64::from(size.height) * 72. / 2540.;
        let min_w = f64::from(minimum.width);
        let min_h = f64::from(minimum.length);
        let max_w = f64::from(maximum.width);
        let max_h = f64::from(maximum.length);
        anyhow::ensure!(
            found != 0
                && [min_w, min_h, max_w, max_h]
                    .iter()
                    .all(|n| n.is_finite() && *n > 0.)
                && width + 0.001 >= min_w
                && height + 0.001 >= min_h
                && width <= max_w + 0.001
                && height <= max_h + 0.001,
            "This paper size is unavailable with the selected printer settings"
        );
        Ok(())
    }
}
fn geometry_matches(size: &Size, media: &Media) -> bool {
    let width = f64::from(size.width);
    let height = f64::from(size.length);
    let points = [
        width,
        height,
        f64::from(size.left),
        f64::from(size.bottom),
        width - f64::from(size.right),
        height - f64::from(size.top),
    ];
    let millimetres = [
        media.width_mm,
        media.height_mm,
        media.left_mm,
        media.bottom_mm,
        media.right_mm,
        media.top_mm,
    ];
    points.iter().zip(millimetres).all(|(&points, mm)| {
        points.is_finite() && points >= 0. && (points * 2540. / 72.).round() == (mm * 100.).round()
    })
}

fn ppd_points(hundredths_mm: u32) -> f32 {
    // CUPS's backend first parses a PPD dimension as f32, then truncates its
    // conversion to IPP hundredths. Millimetre strings such as 125.13 can lose
    // a unit there. Select the adjacent point value that preserves the approved
    // integer. CustomSize's bounds keep this step smaller than one device unit.
    let points = (f64::from(hundredths_mm) * 72. / 2540.) as f32;
    if f64::from(points) * (2540. / 72.) < f64::from(hundredths_mm) {
        points.next_up()
    } else {
        points
    }
}
impl Drop for Marked<'_> {
    fn drop(&mut self) {
        // SAFETY: exactly one close of the object returned by ppdOpenFile.
        unsafe {
            (self.api.close)(self.raw.as_ptr());
        }
    }
}
fn bounded(value: &[c_char]) -> anyhow::Result<String> {
    let end = value
        .iter()
        .position(|b| *b == 0)
        .ok_or_else(|| anyhow::anyhow!("Invalid driver text"))?;
    let bytes = value[..end].iter().map(|b| *b as u8).collect::<Vec<_>>();
    Ok(String::from_utf8(bytes)?)
}
fn choice(value: &Choice) -> anyhow::Result<SettingChoice> {
    let name = bounded(&value.name)?;
    anyhow::ensure!(
        !name.is_empty() && !name.chars().any(char::is_control),
        "Invalid driver choice"
    );
    let label = bounded(&value.text)
        .unwrap_or_else(|_| name.clone())
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>();
    Ok(SettingChoice {
        label: if label.is_empty() {
            name.clone()
        } else {
            label
        },
        value: name,
    })
}
fn escape(queue: &str) -> String {
    queue
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    #[test]
    fn ppd_point_encoding_preserves_every_supported_integer_dimension() {
        for dimension in 1..=1_000_000 {
            let encoded = ppd_points(dimension).to_string();
            // Match the backend's independent string -> f32 -> integer path.
            let parsed: f32 = encoded.parse().unwrap();
            let device_dimension = (f64::from(parsed) * (2540. / 72.)) as u32;
            assert_eq!(device_dimension, dimension, "{dimension} / {encoded}");
        }
    }
    #[test]
    fn approved_fractional_paper_rejects_the_observed_nearby_driver_preset() {
        let media = Media {
            id: "custom_canopa_125.01x225.02mm".into(),
            width_mm: 125.01,
            height_mm: 225.02,
            left_mm: 6.35,
            right_mm: 6.35,
            top_mm: 12.7,
            bottom_mm: 12.7,
        };
        // Public records observed from the actual generated driverless PPD.
        let mut custom = Size {
            width: 354.35907,
            length: 637.852,
            left: 18.,
            bottom: 36.,
            right: 336.35907,
            top: 601.852,
            ..Size::default()
        };
        assert!(geometry_matches(&custom, &media));
        let preset = Size {
            width: 354.33072,
            length: 637.7953,
            left: 9.014_173,
            bottom: 36.,
            right: 345.316_56,
            top: 628.781_1,
            ..Size::default()
        };
        assert!(!geometry_matches(&preset, &media));
        custom.right -= 0.01 * 72. / 25.4;
        assert!(!geometry_matches(&custom, &media));
        custom.right = f32::NAN;
        assert!(!geometry_matches(&custom, &media));
    }
    #[test]
    #[ignore = "requires the installed CUPS PPD runtime; exercised by the Print native gate"]
    fn driver_mapping_conflicts_and_conditional_limits_use_public_cups_records() {
        let api = Api::load().unwrap();
        let options = [
            (c"media", c"custom_canopa_125.13x225.13mm"),
            (c"print-scaling", c"none"),
            (c"PageSize", c"Custom.125.13x225.13mm"),
            (c"copies", c"1"),
            (c"sides", c"one-sided"),
        ]
        .map(|(name, value)| (CString::from(name), CString::from(value)));
        let mut raw_options = option_array(&options);
        for (name, value) in &options {
            // Exercise CUPS's actual binary search, including mixed-case PPD
            // names, rather than testing our sort order against itself.
            let found = unsafe {
                (api.option)(
                    name.as_ptr(),
                    raw_options.len() as c_int,
                    raw_options.as_mut_ptr(),
                )
            };
            assert!(!found.is_null());
            assert_eq!(unsafe { CStr::from_ptr(found) }, value.as_c_str());
        }
        let driver = Driver::from_bytes(
            api._library,
            include_bytes!("../../../../../../scripts/desktop-fixture/print-pdf-advanced.ppd"),
        )
        .unwrap();
        for (field, value, expected) in [
            (SettingField::Source, "manual", "Manual"),
            (SettingField::Type, "photographic-glossy", "Glossy"),
            (SettingField::Quality, "5", "High"),
            (SettingField::Resolution, "600x1200dpi", "600x1200dpi"),
        ] {
            assert_eq!(
                driver.mapped(field, value).unwrap().unwrap().value,
                expected
            );
        }
        assert!(driver.mapped(SettingField::Source, "unrecognized").is_err());
        let marked = driver.open().unwrap();
        marked
            .mark(&[
                (
                    CString::new("InputSlot").unwrap(),
                    CString::new("Manual").unwrap(),
                ),
                (
                    CString::new("MediaType").unwrap(),
                    CString::new("Glossy").unwrap(),
                ),
            ])
            .unwrap();
        assert!(unsafe { (driver.api.conflicts)(marked.raw.as_ptr()) } > 0);
        assert!(
            marked
                .custom_limits(CustomSize::parse("125", "225").unwrap())
                .is_err()
        );
        marked
            .custom_limits(CustomSize::parse("125", "200").unwrap())
            .unwrap();
        driver
            .open()
            .unwrap()
            .custom_limits(CustomSize::parse("125", "225").unwrap())
            .unwrap();
        assert_eq!(escape("Office%<>"), "Office%25%3C%3E");
    }
}
