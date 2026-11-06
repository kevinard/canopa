//! CUPS destination API, loaded only by the isolated printing worker.
mod authentication;
mod connection;
mod discovery;
mod held;
mod identity;
mod job_auth;
mod media;
mod ppd;
mod protocol;
mod recovery;
mod submission;
mod validation;
use super::{Cancellation, Event, document, event};
use super::{CustomSize, Media};
pub(super) use discovery::{capabilities, printers};
pub(super) use held::{authenticate as authenticate_held, list as live_jobs};
use libloading::Library;
pub(super) use media::custom_media;
pub(super) use recovery::recover;
use serde::{Deserialize, Serialize};
use std::{
    ffi::{CString, c_char, c_int, c_void},
    fs::File,
    ptr::{self, NonNull},
};
pub(super) use validation::check_printer;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Destination {
    pub queue: String,
    pub instance: Option<String>,
}
impl Destination {
    pub(crate) fn name(&self) -> String {
        self.instance.as_ref().map_or_else(
            || self.queue.clone(),
            |instance| format!("{}/{instance}", self.queue),
        )
    }
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        for value in std::iter::once(&self.queue).chain(self.instance.iter()) {
            anyhow::ensure!(
                !value.is_empty()
                    && value.len() <= 127
                    && !value
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '/' | '#' | '?' | ' ')),
                "Invalid printer destination"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sides {
    OneSided,
    LongEdge,
    ShortEdge,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobOptions {
    pub copies: u32,
    pub media: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_media: Option<Media>,
    pub sides: Option<Sides>,
    pub color: Option<bool>,
    #[serde(default)]
    pub settings: super::Settings,
}
impl Default for JobOptions {
    fn default() -> Self {
        Self {
            copies: 1,
            media: None,
            custom_media: None,
            sides: None,
            color: None,
            settings: Default::default(),
        }
    }
}
impl JobOptions {
    pub(crate) fn validate_media(&self) -> anyhow::Result<()> {
        if let Some(media) = &self.custom_media {
            anyhow::ensure!(self.media.is_none(), "Conflicting paper choices");
            let size = CustomSize::from_media(media)?;
            anyhow::ensure!(size.matches(media), "Invalid custom paper identity");
        }
        Ok(())
    }
    fn values(&self) -> anyhow::Result<Vec<(CString, CString)>> {
        self.validate_media()?;
        self.settings.validate()?;
        anyhow::ensure!(
            (1..=9999).contains(&self.copies),
            "Invalid printer copy count"
        );
        // Prepared pages and the direct-layout worker already own scaling.
        // CUPS filters must not fit those pages into the printable area again.
        let mut pairs = vec![
            ("copies", self.copies.to_string()),
            ("print-scaling", "none".into()),
        ];
        if let Some(media) = self
            .media
            .as_ref()
            .or(self.custom_media.as_ref().map(|m| &m.id))
        {
            anyhow::ensure!(
                !media.is_empty() && media.len() <= 127 && !media.chars().any(char::is_control),
                "Invalid printer media"
            );
            pairs.push(("media", media.clone()));
        }
        if let Some(sides) = self.sides {
            pairs.push((
                "sides",
                match sides {
                    Sides::OneSided => "one-sided",
                    Sides::LongEdge => "two-sided-long-edge",
                    Sides::ShortEdge => "two-sided-short-edge",
                }
                .into(),
            ));
        }
        if let Some(color) = self.color {
            pairs.push((
                "print-color-mode",
                if color { "color" } else { "monochrome" }.into(),
            ));
        }
        for field in super::SettingField::ALL {
            if let Some(value) = self.settings.get(field) {
                pairs.push((field.ipp().to_str()?, value.clone()));
            }
        }
        pairs
            .into_iter()
            .map(|(key, value)| Ok((CString::new(key)?, CString::new(value)?)))
            .collect()
    }
}

// These layouts and signatures follow the stable CUPS 1.6/1.7 destination API
// from cups/cups.h and cups/http.h; no private CUPS structure is dereferenced.
#[repr(C)]
struct OptionValue {
    name: *mut c_char,
    value: *mut c_char,
}

fn option_array(values: &[(CString, CString)]) -> Vec<OptionValue> {
    // CUPS looks up options with a case-insensitive binary search. Retain the
    // caller's strings, but order every array passed to its public functions.
    let mut pairs = values.iter().collect::<Vec<_>>();
    pairs.sort_unstable_by(|(left, _), (right, _)| {
        left.as_bytes()
            .iter()
            .map(u8::to_ascii_lowercase)
            .cmp(right.as_bytes().iter().map(u8::to_ascii_lowercase))
    });
    pairs
        .into_iter()
        .map(|(name, value)| OptionValue {
            name: name.as_ptr().cast_mut(),
            value: value.as_ptr().cast_mut(),
        })
        .collect()
}
#[repr(C)]
struct RawDestination {
    name: *mut c_char,
    instance: *mut c_char,
    is_default: c_int,
    num_options: c_int,
    options: *mut OptionValue,
}
type Http = *mut c_void;
type Info = *mut c_void;
#[repr(C)]
struct RawJob {
    id: c_int,
    destination: *mut c_char,
    title: *mut c_char,
    user: *mut c_char,
    format: *mut c_char,
    state: c_int,
    size: c_int,
    priority: c_int,
    completed: libc::time_t,
    created: libc::time_t,
    processing: libc::time_t,
}
#[repr(C)]
struct RawMedia {
    media: [c_char; 128],
    width: c_int,
    height: c_int,
    bottom: c_int,
    left: c_int,
    right: c_int,
    top: c_int,
}
// Public cups/pwg.h media-name mapping, borrowed from CUPS thread-local data.
#[repr(C)]
struct RawPwgMedia {
    pwg: *const c_char,
    legacy: *const c_char,
    ppd: *const c_char,
    width: c_int,
    height: c_int,
}

impl Default for RawMedia {
    fn default() -> Self {
        Self {
            media: [0; 128],
            width: 0,
            height: 0,
            bottom: 0,
            left: 0,
            right: 0,
            top: 0,
        }
    }
}

struct Api {
    protocol: protocol::Functions,
    authentication: authentication::Functions,
    job_auth: job_auth::Functions,
    _library: &'static Library,
    limits: crate::print::authentication::ServerLimits,
    server: unsafe extern "C" fn() -> *const c_char,
    user: unsafe extern "C" fn() -> *const c_char,
    port: unsafe extern "C" fn() -> c_int,
    encryption: unsafe extern "C" fn() -> c_int,
    connect: unsafe extern "C" fn(
        *const c_char,
        c_int,
        *mut c_void,
        c_int,
        c_int,
        c_int,
        c_int,
        *mut c_int,
    ) -> Http,
    timeout: unsafe extern "C" fn(
        Http,
        f64,
        Option<unsafe extern "C" fn(Http, *mut c_void) -> c_int>,
        *mut c_void,
    ),
    http_status: unsafe extern "C" fn(Http) -> c_int,
    close: unsafe extern "C" fn(Http),
    named: unsafe extern "C" fn(Http, *const c_char, *const c_char) -> *mut RawDestination,
    free_destinations: unsafe extern "C" fn(c_int, *mut RawDestination),
    copy_info: unsafe extern "C" fn(Http, *mut RawDestination) -> Info,
    free_info: unsafe extern "C" fn(Info),
    supported: unsafe extern "C" fn(
        Http,
        *mut RawDestination,
        Info,
        *const c_char,
        *const c_char,
    ) -> c_int,
    create: unsafe extern "C" fn(
        Http,
        *mut RawDestination,
        Info,
        *mut c_int,
        *const c_char,
        c_int,
        *mut OptionValue,
    ) -> c_int,
    start: unsafe extern "C" fn(
        Http,
        *mut RawDestination,
        Info,
        c_int,
        *const c_char,
        *const c_char,
        c_int,
        *mut OptionValue,
        c_int,
    ) -> c_int,
    write: unsafe extern "C" fn(Http, *const c_char, usize) -> c_int,
    cancel: unsafe extern "C" fn(Http, *mut RawDestination, c_int) -> c_int,
    jobs: unsafe extern "C" fn(Http, *mut *mut RawJob, *const c_char, c_int, c_int) -> c_int,
    free_jobs: unsafe extern "C" fn(c_int, *mut RawJob),
    destinations: unsafe extern "C" fn(Http, *mut *mut RawDestination) -> c_int,
    option: unsafe extern "C" fn(*const c_char, c_int, *mut OptionValue) -> *const c_char,
    last_error: unsafe extern "C" fn() -> c_int,
    find_supported:
        unsafe extern "C" fn(Http, *mut RawDestination, Info, *const c_char) -> *mut c_void,
    find_default:
        unsafe extern "C" fn(Http, *mut RawDestination, Info, *const c_char) -> *mut c_void,
    attribute_tag: unsafe extern "C" fn(*mut c_void) -> c_int,
    attribute_count: unsafe extern "C" fn(*mut c_void) -> c_int,
    attribute_string: unsafe extern "C" fn(*mut c_void, c_int, *mut *const c_char) -> *const c_char,
    attribute_range: unsafe extern "C" fn(*mut c_void, c_int, *mut c_int) -> c_int,
    media_count: unsafe extern "C" fn(Http, *mut RawDestination, Info, u32) -> c_int,
    media_by_name: unsafe extern "C" fn(
        Http,
        *mut RawDestination,
        Info,
        *const c_char,
        u32,
        *mut RawMedia,
    ) -> c_int,
    media_for_ppd: unsafe extern "C" fn(*const c_char) -> *const RawPwgMedia,
    get_fd: unsafe extern "C" fn(Http, *const c_char, c_int) -> c_int,
    attribute_integer: unsafe extern "C" fn(*mut c_void, c_int) -> c_int,
    attribute_resolution: unsafe extern "C" fn(*mut c_void, c_int, *mut c_int, *mut c_int) -> c_int,
    conflicts: unsafe extern "C" fn(
        Http,
        *mut RawDestination,
        Info,
        c_int,
        *mut OptionValue,
        *const c_char,
        *const c_char,
        *mut c_int,
        *mut *mut OptionValue,
        *mut c_int,
        *mut *mut OptionValue,
    ) -> c_int,
}
impl Api {
    fn load() -> anyhow::Result<Self> {
        let runtime = super::runtime::library()?;
        let library = &runtime.library;
        // SAFETY: the retained selected CUPS client is loaded only in this standalone
        // worker. Each symbol uses its installed public C header signature, and
        // the Library is retained until all pointers and CUPS objects are gone.
        unsafe {
            Ok(Self {
                limits: runtime.limits,
                protocol: protocol::Functions::load(library)?,
                authentication: authentication::Functions::load(library)?,
                job_auth: job_auth::Functions::load(library)?,
                server: *library.get(b"cupsServer\0")?,
                user: *library.get(b"cupsUser\0")?,
                port: *library.get(b"ippPort\0")?,
                encryption: *library.get(b"cupsEncryption\0")?,
                connect: *library.get(b"httpConnect2\0")?,
                timeout: *library.get(b"httpSetTimeout\0")?,
                http_status: *library.get(b"httpGetStatus\0")?,
                close: *library.get(b"httpClose\0")?,
                named: *library.get(b"cupsGetNamedDest\0")?,
                free_destinations: *library.get(b"cupsFreeDests\0")?,
                copy_info: *library.get(b"cupsCopyDestInfo\0")?,
                free_info: *library.get(b"cupsFreeDestInfo\0")?,
                supported: *library.get(b"cupsCheckDestSupported\0")?,
                create: *library.get(b"cupsCreateDestJob\0")?,
                start: *library.get(b"cupsStartDestDocument\0")?,
                write: *library.get(b"cupsWriteRequestData\0")?,
                cancel: *library.get(b"cupsCancelDestJob\0")?,
                jobs: *library.get(b"cupsGetJobs2\0")?,
                free_jobs: *library.get(b"cupsFreeJobs\0")?,
                destinations: *library.get(b"cupsGetDests2\0")?,
                option: *library.get(b"cupsGetOption\0")?,
                last_error: *library.get(b"cupsLastError\0")?,
                find_supported: *library.get(b"cupsFindDestSupported\0")?,
                find_default: *library.get(b"cupsFindDestDefault\0")?,
                attribute_tag: *library.get(b"ippGetValueTag\0")?,
                attribute_count: *library.get(b"ippGetCount\0")?,
                attribute_string: *library.get(b"ippGetString\0")?,
                attribute_range: *library.get(b"ippGetRange\0")?,
                media_count: *library.get(b"cupsGetDestMediaCount\0")?,
                media_by_name: *library.get(b"cupsGetDestMediaByName\0")?,
                media_for_ppd: *library.get(b"pwgMediaForPPD\0")?,
                get_fd: *library.get(b"cupsGetFd\0")?,
                attribute_integer: *library.get(b"ippGetInteger\0")?,
                attribute_resolution: *library.get(b"ippGetResolution\0")?,
                conflicts: *library.get(b"cupsCopyDestConflicts\0")?,
                _library: library,
            })
        }
    }
}
struct Session {
    api: Api,
    http: NonNull<c_void>,
    authentication: Box<authentication::Authentication>,
}
impl Session {
    fn open() -> anyhow::Result<Self> {
        Self::open_protected(None)
    }
    fn open_protected(protection: Option<c_int>) -> anyhow::Result<Self> {
        let api = Api::load()?;
        let mut cancelled = 0;
        // SAFETY: all functions belong to the retained API; returned server text
        // is CUPS-owned. The cancel integer remains live throughout connect.
        let http = unsafe {
            (api.connect)(
                (api.server)(),
                (api.port)(),
                ptr::null_mut(),
                libc::AF_UNSPEC,
                protection.unwrap_or_else(|| (api.encryption)()),
                1,
                3000,
                &mut cancelled,
            )
        };
        let http =
            NonNull::new(http).ok_or_else(|| anyhow::anyhow!("Print scheduler unavailable"))?;
        // SAFETY: this is a live connection and no callback/context is installed.
        unsafe {
            (api.timeout)(http.as_ptr(), 3.0, None, ptr::null_mut());
        }
        let authentication = match authentication::Authentication::install(&api, http.as_ptr()) {
            Ok(authentication) => authentication,
            Err(error) => {
                // SAFETY: connect returned this uniquely owned live connection.
                unsafe {
                    (api.close)(http.as_ptr());
                }
                return Err(error);
            }
        };
        Ok(Self {
            api,
            http,
            authentication,
        })
    }
    fn destination(
        &self,
        destination: &Destination,
        capabilities: bool,
    ) -> anyhow::Result<Selected<'_>> {
        destination.validate()?;
        let queue = CString::new(destination.queue.as_str())?;
        let instance = destination
            .instance
            .as_deref()
            .map(CString::new)
            .transpose()?;
        // SAFETY: input strings remain live for this call; the result is owned
        // by Selected and freed once with cupsFreeDests.
        let raw = unsafe {
            (self.api.named)(
                self.http.as_ptr(),
                queue.as_ptr(),
                instance.as_ref().map_or(ptr::null(), |s| s.as_ptr()),
            )
        };
        let raw =
            NonNull::new(raw).ok_or_else(|| anyhow::anyhow!("Printer destination unavailable"))?;
        let mut selected = Selected {
            session: self,
            raw,
            info: None,
        };
        if capabilities {
            // SAFETY: the connection/destination are live; the result is owned
            // by Selected and freed before its destination and connection.
            selected.info =
                NonNull::new(unsafe { (self.api.copy_info)(self.http.as_ptr(), raw.as_ptr()) });
            anyhow::ensure!(selected.info.is_some(), "Printer capabilities unavailable");
        }
        Ok(selected)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.authentication.disable();
        // SAFETY: uniquely owned live connection, released before the library.
        unsafe {
            (self.api.close)(self.http.as_ptr());
        }
    }
}
struct Selected<'a> {
    session: &'a Session,
    raw: NonNull<RawDestination>,
    info: Option<NonNull<c_void>>,
}
impl Drop for Selected<'_> {
    fn drop(&mut self) {
        // SAFETY: these objects were allocated by this retained API; capability
        // information is retired before the destination it describes.
        unsafe {
            if let Some(info) = self.info {
                (self.session.api.free_info)(info.as_ptr());
            }
            (self.session.api.free_destinations)(1, self.raw.as_ptr());
        }
    }
}
fn succeeded(status: c_int) -> bool {
    (0..0x100).contains(&status)
}

pub(super) fn cancel(destination: &Destination, job: i32) -> anyhow::Result<()> {
    anyhow::ensure!(job > 0, "Invalid owned print job");
    let session = Session::open()?;
    let selected = session.destination(destination, false)?;
    // SAFETY: the positive owned job ID is sent on a live destination connection.
    let status = unsafe { (session.api.cancel)(session.http.as_ptr(), selected.raw.as_ptr(), job) };
    if succeeded(status) {
        return Ok(());
    }
    // Cleanup can race the worker's own RAII cancellation or a lost final
    // acknowledgement. A terminal owned job needs no second cancellation.
    let queue = CString::new(destination.queue.as_str())?;
    let mut raw = ptr::null_mut();
    // SAFETY: the initialized result pointer is filled by CUPS with a counted
    // array of public cups_job_t records for this user's selected queue.
    let count =
        unsafe { (session.api.jobs)(session.http.as_ptr(), &mut raw, queue.as_ptr(), 1, -1) };
    struct Jobs<'a> {
        api: &'a Api,
        count: c_int,
        raw: *mut RawJob,
    }
    impl Drop for Jobs<'_> {
        fn drop(&mut self) {
            if !self.raw.is_null() && self.count >= 0 {
                // SAFETY: exactly the allocation/count returned by cupsGetJobs2.
                unsafe {
                    (self.api.free_jobs)(self.count, self.raw);
                }
            }
        }
    }
    let jobs = Jobs {
        api: &session.api,
        count,
        raw,
    };
    anyhow::ensure!(
        (1..=4096).contains(&count) && !raw.is_null(),
        "Owned print job retirement was not confirmed"
    );
    // SAFETY: CUPS supplies count initialized public records; Jobs keeps them
    // allocated until after this bounded search and frees them on every return.
    let records = unsafe { std::slice::from_raw_parts(jobs.raw, count as usize) };
    anyhow::ensure!(
        records
            .iter()
            .any(|record| record.id == job && [7, 8, 9].contains(&record.state)),
        "Owned print job retirement was not confirmed"
    );
    Ok(())
}
struct PartialJob {
    id: i32,
    committed: bool,
    withdrawn: bool,
    journal: std::rc::Rc<recovery::Journal>,
    connections: Option<connection::Connections>,
}
impl Drop for PartialJob {
    fn drop(&mut self) {
        // This is automatic error retirement, not another user decision. Keep
        // the approved account/scope cache available, but prohibit new prompts.
        let _credentials = crate::print::authentication::CacheOnly::enter();
        if !self.committed {
            self.journal
                .retire(self.id, &mut self.connections, self.withdrawn);
        }
    }
}

pub(super) fn submit(
    file: File,
    destination: Destination,
    title: String,
    options: JobOptions,
    cancellation: &Cancellation,
    registration: Option<super::RecoveryRegistration>,
) -> anyhow::Result<Event> {
    cancellation.check()?;
    let document = document::verify(&file)?;
    anyhow::ensure!(
        title.len() <= 4096 && !title.contains('\0'),
        "Invalid print title"
    );
    let session = Session::open()?;
    let selected = session.destination(&destination, true)?;
    let values = validation::selected(&selected, &options)?;
    let authenticated = job_auth::Prepared::prepare(&selected, &destination)?;
    let mut raw_options = option_array(&values);
    let http = session.http.as_ptr();
    let dest = selected.raw.as_ptr();
    let info = selected.info.expect("requested capabilities").as_ptr();
    cancellation.check()?;
    let journal = recovery::Journal::begin(&session, &destination, &title, registration)?;
    let title = CString::new(journal.name())?;
    let mut job = PartialJob {
        id: 0,
        committed: false,
        withdrawn: false,
        journal,
        connections: None,
    };
    let mut id = 0;
    // SAFETY: all objects and the mutable ID/options array remain live for call.
    let creation = {
        // Only authentication during Create may change the provisional owner.
        // Drop this scope before recording the ID or starting the document.
        let _account = session.authentication.creating(Some(job.journal.clone()))?;
        if let Some(authenticated) = authenticated {
            authenticated.create(&selected, &title, &mut raw_options, &mut id)
        } else {
            Ok(unsafe {
                (session.api.create)(
                    http,
                    dest,
                    info,
                    &mut id,
                    title.as_ptr(),
                    raw_options.len() as c_int,
                    raw_options.as_mut_ptr(),
                )
            })
        }
    };
    job.id = id;
    let status = creation?;
    anyhow::ensure!(
        succeeded(status) && id > 0,
        "Print job creation was not acknowledged"
    );
    job.journal.created(id)?;
    event(&Event::Created { job: id })?;
    cancellation.check()?;
    if let Err(error) = submission::send(
        &file,
        &document,
        &selected,
        &options,
        cancellation,
        &mut job,
    ) {
        job.withdrawn = cancellation.withdrawn();
        return Err(error);
    }
    // The scheduler's complete HTTP/IPP acknowledgement commits the job.
    // Later receipt bookkeeping must neither cancel it nor report a failure
    // that could lead the caller to submit another copy.
    job.committed = true;
    match job.journal.accepted(id) {
        Ok(()) => job.journal.finish(),
        Err(error) => eprintln!(
            "level=warn component=print event=accepted_printer_receipt_unconfirmed error={error}"
        ),
    }
    Ok(Event::Accepted { job: id })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submission_options_are_explicit_known_fields_without_arbitrary_cups_extensions() {
        let values = JobOptions {
            copies: 2,
            media: Some("iso_a4_210x297mm".into()),
            custom_media: None,
            sides: Some(Sides::LongEdge),
            color: Some(false),
            settings: Default::default(),
        }
        .values()
        .unwrap();
        let values = values
            .iter()
            .map(|(key, value)| (key.to_str().unwrap(), value.to_str().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            [
                ("copies", "2"),
                ("print-scaling", "none"),
                ("media", "iso_a4_210x297mm"),
                ("sides", "two-sided-long-edge"),
                ("print-color-mode", "monochrome")
            ]
        );
        assert!(serde_json::from_value::<JobOptions>(serde_json::json!({ "copies": 1, "media": null, "sides": null, "color": null, "cups-device-uri": "ipp://not-approved" })).is_err());
    }

    #[test]
    fn invalid_device_options_and_destinations_fail_before_opening_cups() {
        for copies in [0, 10_000] {
            assert!(
                JobOptions {
                    copies,
                    ..JobOptions::default()
                }
                .values()
                .is_err()
            );
        }
        assert!(
            JobOptions {
                media: Some("bad\0media".into()),
                ..JobOptions::default()
            }
            .values()
            .is_err()
        );
        for queue in ["", "ipp://not-approved/print", "bad queue", "line\nbreak"] {
            assert!(
                Destination {
                    queue: queue.into(),
                    instance: None
                }
                .validate()
                .is_err()
            );
        }
        assert!(
            Destination {
                queue: "Office".into(),
                instance: Some("Duplex".into())
            }
            .validate()
            .is_ok()
        );
    }

    #[test]
    fn only_ipp_success_class_acknowledges_submission() {
        assert!(succeeded(0));
        assert!(succeeded(1));
        for status in [-1, 0x300, 0x400, 0x500] {
            assert!(!succeeded(status));
        }
    }
}
