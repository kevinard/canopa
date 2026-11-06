//! Create-Job with literal IPP authentication fields. CUPS's option-string
//! encoder cannot preserve every single-field password containing punctuation.
use super::*;
use crate::print::authentication::{Credentials, Requirement};
use std::ffi::CStr;

#[derive(Clone, Copy)]
pub(super) struct Functions {
    new: unsafe extern "C" fn(c_int) -> *mut c_void,
    delete: unsafe extern "C" fn(*mut c_void),
    version: unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int,
    string: unsafe extern "C" fn(
        *mut c_void,
        c_int,
        c_int,
        *const c_char,
        *const c_char,
        *const c_char,
    ) -> *mut c_void,
    strings: unsafe extern "C" fn(
        *mut c_void,
        c_int,
        c_int,
        *const c_char,
        c_int,
        *const c_char,
        *const *const c_char,
    ) -> *mut c_void,
    encode: unsafe extern "C" fn(*mut c_void, c_int, *mut OptionValue, c_int),
    request: unsafe extern "C" fn(Http, *mut c_void, *const c_char) -> *mut c_void,
    status: unsafe extern "C" fn(*mut c_void) -> c_int,
    find: unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> *mut c_void,
    group: unsafe extern "C" fn(*mut c_void) -> c_int,
}
impl Functions {
    pub fn load(library: &Library) -> anyhow::Result<Self> {
        // SAFETY: public cups/ipp header signatures. Api retains the library.
        unsafe {
            Ok(Self {
                new: *library.get(b"ippNewRequest\0")?,
                delete: *library.get(b"ippDelete\0")?,
                version: *library.get(b"ippSetVersion\0")?,
                string: *library.get(b"ippAddString\0")?,
                strings: *library.get(b"ippAddStrings\0")?,
                encode: *library.get(b"cupsEncodeOptions2\0")?,
                request: *library.get(b"cupsDoRequest\0")?,
                status: *library.get(b"ippGetStatusCode\0")?,
                find: *library.get(b"ippFindAttribute\0")?,
                group: *library.get(b"ippGetGroupTag\0")?,
            })
        }
    }
}
struct Message {
    raw: *mut c_void,
    delete: unsafe extern "C" fn(*mut c_void),
}
impl Drop for Message {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                (self.delete)(self.raw);
            }
        }
    }
}
impl Message {
    fn new(raw: *mut c_void, functions: Functions) -> anyhow::Result<Self> {
        anyhow::ensure!(!raw.is_null(), "Printer authentication request unavailable");
        Ok(Self {
            raw,
            delete: functions.delete,
        })
    }
    fn take(mut self) -> *mut c_void {
        std::mem::replace(&mut self.raw, ptr::null_mut())
    }
}
pub(super) struct Prepared {
    uri: CString,
    resource: CString,
    version: (c_int, c_int),
    credentials: Credentials,
}
impl Prepared {
    pub fn prepare(
        selected: &Selected<'_>,
        destination: &Destination,
    ) -> anyhow::Result<Option<Self>> {
        let Requirement::Fields { fields } = discovery::authentication(selected)? else {
            return Ok(None);
        };
        let uri = discovery::metadata(selected, c"printer-uri-supported")?
            .ok_or_else(|| anyhow::anyhow!("Printer authentication URI unavailable"))?;
        let parsed = url::Url::parse(&uri)?;
        anyhow::ensure!(
            matches!(parsed.scheme(), "ipp" | "ipps" | "http" | "https")
                && parsed.host_str().is_some()
                && parsed.username().is_empty()
                && parsed.password().is_none()
                && parsed.query().is_none()
                && parsed.fragment().is_none()
                && parsed.path().starts_with('/')
                && parsed.path().len() <= 1024,
            "Invalid printer authentication URI"
        );
        let resource = CString::new(parsed.path())?;
        let versions = discovery::versions(selected)?;
        let version = if versions
            .iter()
            .any(|v| matches!(v.as_str(), "2.0" | "2.1" | "2.2"))
        {
            (2, 0)
        } else if versions.iter().any(|v| v == "1.1") {
            (1, 1)
        } else {
            anyhow::bail!("Printer authentication IPP version unavailable");
        };
        let credentials = selected
            .session
            .authentication
            .queue_credentials(destination, fields)?;
        Ok(Some(Self {
            uri: CString::new(uri)?,
            resource,
            version,
            credentials,
        }))
    }
    pub fn create(
        self,
        selected: &Selected<'_>,
        title: &CStr,
        options: &mut [OptionValue],
        job: &mut c_int,
    ) -> anyhow::Result<c_int> {
        let api = &selected.session.api;
        let functions = api.job_auth;
        // SAFETY: CREATE_JOB=5 is public IPP; the request is uniquely owned.
        let request = Message::new(unsafe { (functions.new)(5) }, functions)?;
        anyhow::ensure!(
            unsafe { (functions.version)(request.raw, self.version.0, self.version.1) } != 0,
            "Cannot set authentication request version"
        );
        // Same public operation attributes as cupsCreateDestJob, without reading
        // its opaque destination-info object. No request is retried by Canopa.
        for (name, tag, value) in [
            (c"printer-uri", 0x45, self.uri.as_ptr()),
            (c"requesting-user-name", 0x42, unsafe { (api.user)() }),
            (c"job-name", 0x42, title.as_ptr()),
        ] {
            anyhow::ensure!(
                !unsafe {
                    (functions.string)(request.raw, 1, tag, name.as_ptr(), ptr::null(), value)
                }
                .is_null(),
                "Cannot create authentication attributes"
            );
        }
        for group in [1, 2, 6] {
            unsafe {
                (functions.encode)(
                    request.raw,
                    options.len() as c_int,
                    options.as_mut_ptr(),
                    group,
                );
            }
        }
        let values = self
            .credentials
            .values
            .into_iter()
            .map(|v| v.c_string())
            .collect::<anyhow::Result<Vec<_>>>()?;
        let pointers = values
            .iter()
            .map(|v| v.bytes().as_ptr().cast::<c_char>())
            .collect::<Vec<_>>();
        // Literal, ordered TEXT values. The IPP API copies them; Secret clears
        // our own copies on return. No credentials enter JobOptions or journal.
        anyhow::ensure!(
            !unsafe {
                (functions.strings)(
                    request.raw,
                    2,
                    0x41,
                    c"auth-info".as_ptr(),
                    pointers.len() as c_int,
                    ptr::null(),
                    pointers.as_ptr(),
                )
            }
            .is_null(),
            "Cannot create literal authentication fields"
        );
        // cupsDoRequest takes the request's ownership even on error. Its HTTP
        // object remains this selected session; a URI never chooses a new host.
        let response = Message::new(
            unsafe {
                (functions.request)(
                    selected.session.http.as_ptr(),
                    request.take(),
                    self.resource.as_ptr(),
                )
            },
            functions,
        )?;
        anyhow::ensure!(
            unsafe { (api.http_status)(selected.session.http.as_ptr()) } == 200,
            "Printer authentication creation was not acknowledged"
        );
        let status = unsafe { (functions.status)(response.raw) };
        let attribute = unsafe { (functions.find)(response.raw, c"job-id".as_ptr(), 0x21) };
        if !attribute.is_null() {
            anyhow::ensure!(
                unsafe {
                    (functions.group)(attribute) == 2 && (api.attribute_count)(attribute) == 1
                },
                "Invalid authenticated job receipt"
            );
            *job = unsafe { (api.attribute_integer)(attribute, 0) };
        }
        Ok(status)
    }
}
