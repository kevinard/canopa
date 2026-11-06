//! Public CUPS password callback. Its userdata and returned password outlive
//! every synchronous CUPS request on this session; no console fallback exists.
use super::*;
use crate::print::authentication::{self, Challenge, Endpoint, Kind, Secret, Transport};
use std::{
    cell::{Cell, RefCell},
    ffi::CStr,
    rc::Rc,
};

#[derive(Default)]
struct Deferring {
    active: Cell<bool>,
    rejected: Cell<bool>,
    resource: RefCell<Option<String>>,
}
pub(super) struct Deferral(Rc<Deferring>);
impl Deferral {
    pub fn resource(&self) -> Option<String> {
        self.0.resource.borrow().clone()
    }
    pub fn rejected(&self) -> bool {
        self.0.rejected.get()
    }
}
impl Drop for Deferral {
    fn drop(&mut self) {
        self.0.active.set(false);
    }
}

#[derive(Default)]
struct Creating {
    active: bool,
    journal: Option<Rc<recovery::Journal>>,
}

/// Owns its state independently of the callback Box: no Rust borrow of that
/// Box may remain live while CUPS invokes it through its userdata pointer.
pub(super) struct CreationScope(Rc<RefCell<Creating>>);
impl Drop for CreationScope {
    fn drop(&mut self) {
        *self.0.borrow_mut() = Creating::default();
    }
}

type Callback = unsafe extern "C" fn(
    *const c_char,
    Http,
    *const c_char,
    *const c_char,
    *mut c_void,
) -> *const c_char;
#[derive(Clone, Copy)]
pub(super) struct Functions {
    pub(super) authenticate: unsafe extern "C" fn(Http, *const c_char, *const c_char) -> c_int,
    status: unsafe extern "C" fn(Http) -> c_int,
    callback: unsafe extern "C" fn(Option<Callback>, *mut c_void),
    user: unsafe extern "C" fn(*const c_char),
    fd: unsafe extern "C" fn(Http) -> c_int,
    reconnect: unsafe extern "C" fn(Http, c_int, *mut c_int) -> c_int,
    encrypted: unsafe extern "C" fn(Http) -> c_int,
    encryption: unsafe extern "C" fn(Http) -> c_int,
    set_encryption: unsafe extern "C" fn(Http, c_int) -> c_int,
    subfield: unsafe extern "C" fn(Http, c_int, *const c_char, *mut c_char, c_int) -> *mut c_char,
}
impl Functions {
    pub fn load(library: &Library) -> anyhow::Result<Self> {
        // SAFETY: public cups/http headers; Api retains the library for process
        // lifetime, including callbacks registered in CUPS's thread-local state.
        unsafe {
            Ok(Self {
                authenticate: *library.get(b"cupsDoAuthentication\0")?,
                status: *library.get(b"httpGetStatus\0")?,
                callback: *library.get(b"cupsSetPasswordCB2\0")?,
                user: *library.get(b"cupsSetUser\0")?,
                fd: *library.get(b"httpGetFd\0")?,
                reconnect: *library.get(b"httpReconnect2\0")?,
                encrypted: *library.get(b"httpIsEncrypted\0")?,
                encryption: *library.get(b"httpGetEncryption\0")?,
                set_encryption: *library.get(b"httpEncryption\0")?,
                subfield: *library.get(b"httpGetSubField2\0")?,
            })
        }
    }
}
pub(super) struct Authentication {
    functions: Functions,
    limits: authentication::ServerLimits,
    http: Http,
    server: String,
    port: u16,
    suggested_user: String,
    channel: Option<Rc<authentication::Channel>>,
    attempts: Cell<u8>,
    refused: bool,
    password: Option<Secret>,
    creating: Rc<RefCell<Creating>>,
    deferring: Rc<Deferring>,
}
impl Authentication {
    pub fn attempts(&self) -> u8 {
        self.attempts.get()
    }
    pub fn continue_after(&self, attempts: u8) {
        self.attempts.set(attempts);
    }
    pub fn defer(&self) -> anyhow::Result<Deferral> {
        anyhow::ensure!(
            !self.deferring.active.replace(true),
            "Printer authentication already deferred"
        );
        self.deferring.rejected.set(false);
        self.deferring.resource.borrow_mut().take();
        Ok(Deferral(Rc::clone(&self.deferring)))
    }
    pub fn activate(&self) {
        // Registration itself never invokes the callback. No borrow of this
        // allocation is held across a later foreign request.
        unsafe {
            (self.functions.callback)(Some(callback), (self as *const Self).cast_mut().cast());
        }
    }
    pub fn protection(&self) -> c_int {
        unsafe {
            let policy = (self.functions.encryption)(self.http);
            if (self.functions.encrypted)(self.http) != 0 {
                policy.max(2)
            } else {
                policy
            }
        }
    }
    pub fn creating(
        &self,
        journal: Option<Rc<recovery::Journal>>,
    ) -> anyhow::Result<CreationScope> {
        let mut state = self.creating.borrow_mut();
        anyhow::ensure!(!state.active, "Printer creation already active");
        *state = Creating {
            active: true,
            journal,
        };
        Ok(CreationScope(Rc::clone(&self.creating)))
    }
    pub fn queue_credentials(
        &self,
        destination: &Destination,
        fields: Vec<crate::print::authentication::Field>,
    ) -> anyhow::Result<crate::print::authentication::Credentials> {
        let channel = self
            .channel
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Printer credentials required"))?;
        let request = Challenge {
            version: 1,
            interactive: true,
            id: 0,
            endpoint: self.endpoint()?,
            kind: Kind::Queue {
                destination: destination.clone(),
                fields,
            },
            attempt: 1,
            suggested_user: self.suggested_user.clone(),
        };
        channel
            .request(request)?
            .ok_or_else(|| anyhow::anyhow!("Printer authentication cancelled"))
    }
    fn endpoint(&self) -> anyhow::Result<Endpoint> {
        // CUPS closes the socket before invoking its password callback. Open
        // that same public HTTP object before inspecting the actual peer;
        // accepting a missing descriptor would bypass the protection check.
        if unsafe { (self.functions.fd)(self.http) } < 0 {
            let mut cancel = 0;
            anyhow::ensure!(
                unsafe { (self.functions.reconnect)(self.http, 3000, &mut cancel) } == 0,
                "Authentication connection unavailable"
            );
        }
        // SAFETY: public getters on the retained HTTP connection.
        let transport = unsafe {
            transport(
                (self.functions.fd)(self.http),
                (self.functions.encrypted)(self.http) != 0,
            )?
        };
        if transport == Transport::Tls {
            // Preserve direct-TLS policy; forbid a later unencrypted reconnect.
            unsafe {
                if (self.functions.encryption)(self.http) < 2 {
                    anyhow::ensure!(
                        (self.functions.set_encryption)(self.http, 2) == 0,
                        "Cannot require encrypted authentication"
                    );
                }
            }
        }
        Ok(Endpoint {
            server: self.server.clone(),
            port: self.port,
            transport,
        })
    }
    pub fn install(api: &Api, http: Http) -> anyhow::Result<Box<Self>> {
        // Retain an already established TLS connection's protection even when
        // CUPS disconnects before the first callback. Do not replace ALWAYS.
        unsafe {
            if (api.authentication.encrypted)(http) != 0
                && (api.authentication.encryption)(http) < 2
            {
                anyhow::ensure!(
                    (api.authentication.set_encryption)(http, 2) == 0,
                    "Cannot retain encrypted authentication"
                );
            }
        }
        // SAFETY: copied bounded thread-local CUPS configuration strings.
        let (server, user, port) = unsafe {
            (
                text((api.server)(), 4096)?,
                text((api.user)(), 127)?,
                (api.port)(),
            )
        };
        let mut value = Box::new(Self {
            functions: api.authentication,
            limits: api.limits,
            http,
            server,
            port: port.try_into()?,
            suggested_user: user,
            channel: authentication::current(),
            attempts: Cell::new(0),
            refused: false,
            password: None,
            creating: Rc::default(),
            deferring: Rc::default(),
        });
        // SAFETY: the Box's allocation is stable. Session keeps it alive until
        // after all API calls and disables this callback before closing HTTP.
        unsafe {
            (value.functions.callback)(Some(callback), (&mut *value as *mut Self).cast());
        }
        Ok(value)
    }
    pub fn disable(&self) {
        // NULL would restore CUPS's console callback. Always install an explicit
        // refusal instead, including during destruction and unattended work.
        unsafe {
            (self.functions.callback)(Some(refuse), ptr::null_mut());
        }
    }
    fn challenge(&mut self, resource: *const c_char) -> anyhow::Result<*const c_char> {
        anyhow::ensure!(!self.refused, "Authentication already refused");
        if self.deferring.active.get() {
            anyhow::ensure!(
                unsafe { (self.functions.status)(self.http) } == 401,
                "Unexpected printer authentication state"
            );
            *self.deferring.resource.borrow_mut() = Some(unsafe { text(resource, 1024)? });
            self.deferring.rejected.set(true);
            return Ok(ptr::null());
        }
        self.attempts.set(self.attempts.get() + 1);
        anyhow::ensure!(
            self.attempts.get() <= authentication::MAX_ATTEMPTS,
            "Too many server authentication attempts"
        );
        let channel = self
            .channel
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Print server authentication required"))?;
        // SAFETY: the callback is called synchronously for this retained HTTP
        // object; public getters do not transfer ownership of the socket.
        let endpoint = self.endpoint()?;
        let mut realm = [0 as c_char; 257];
        // HTTP_FIELD_WWW_AUTHENTICATE=26 in the public CUPS 2.x enum.
        let found = unsafe {
            (self.functions.subfield)(
                self.http,
                26,
                c"realm".as_ptr(),
                realm.as_mut_ptr(),
                realm.len() as c_int,
            )
        };
        let realm = if found.is_null() {
            String::new()
        } else {
            let end = realm
                .iter()
                .position(|v| *v == 0)
                .ok_or_else(|| anyhow::anyhow!("Invalid authentication realm"))?;
            let bytes = realm[..end].iter().map(|v| *v as u8).collect::<Vec<_>>();
            String::from_utf8(bytes)?
        };
        let request = Challenge {
            version: 1,
            interactive: true,
            id: 0,
            endpoint,
            kind: Kind::Server {
                realm,
                limits: self.limits,
                resource: unsafe { text(resource, 1024)? },
            },
            attempt: self.attempts.get(),
            suggested_user: self.suggested_user.clone(),
        };
        let Some(credentials) = channel.request(request)? else {
            self.refused = true;
            return Ok(ptr::null());
        };
        let mut fields = credentials.values.into_iter();
        let username = fields
            .next()
            .ok_or_else(|| anyhow::anyhow!("Missing authentication username"))?
            .c_string()?;
        let password = fields
            .next()
            .ok_or_else(|| anyhow::anyhow!("Missing authentication password"))?
            .c_string()?;
        if let Some(journal) = &self.creating.borrow().journal {
            // The callback runs after an explicit authentication challenge and
            // before CUPS can retry Create with this principal. A durable write
            // failure returns refusal; never permit an unrecorded owner change.
            journal.creation_user(&username.bytes()[..username.bytes().len() - 1])?;
        }
        // SAFETY: validated UTF-8/NUL-free, bounded username with a terminator;
        // cupsSetUser copies it before the secret allocation is dropped.
        unsafe {
            (self.functions.user)(username.bytes().as_ptr().cast());
        }
        self.password = Some(password);
        Ok(self
            .password
            .as_ref()
            .expect("assigned password")
            .bytes()
            .as_ptr()
            .cast())
    }
}
impl Drop for Authentication {
    fn drop(&mut self) {
        self.disable();
    }
}
unsafe extern "C" fn refuse(
    _: *const c_char,
    _: Http,
    _: *const c_char,
    _: *const c_char,
    _: *mut c_void,
) -> *const c_char {
    ptr::null()
}
unsafe extern "C" fn callback(
    _: *const c_char,
    http: Http,
    _: *const c_char,
    resource: *const c_char,
    data: *mut c_void,
) -> *const c_char {
    if data.is_null() {
        return ptr::null();
    }
    // SAFETY: install registers a stable Box for this thread's retained session;
    // CUPS calls it synchronously. Never unwind a Rust panic into the C library.
    let value = unsafe { &mut *data.cast::<Authentication>() };
    if http != value.http {
        value.refused = true;
        return ptr::null();
    }
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| value.challenge(resource))) {
        Ok(Ok(password)) => password,
        _ => {
            value.refused = true;
            ptr::null()
        }
    }
}
unsafe fn text(value: *const c_char, maximum: usize) -> anyhow::Result<String> {
    anyhow::ensure!(!value.is_null(), "Missing authentication metadata");
    // SAFETY: caller supplies CUPS-owned NUL-terminated metadata.
    let value = unsafe { CStr::from_ptr(value) };
    anyhow::ensure!(
        value.to_bytes().len() <= maximum,
        "Authentication metadata exceeds limits"
    );
    Ok(value.to_str()?.to_owned())
}
unsafe fn transport(fd: c_int, encrypted: bool) -> anyhow::Result<Transport> {
    // SAFETY: initialized public sockaddr storage and writable length. getpeername
    // validates the borrowed integer socket descriptor owned by CUPS.
    let mut address: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
    let mut length = std::mem::size_of_val(&address) as libc::socklen_t;
    anyhow::ensure!(
        fd >= 0
            && unsafe {
                libc::getpeername(
                    fd,
                    (&mut address as *mut libc::sockaddr_storage).cast(),
                    &mut length,
                )
            } == 0,
        "Authentication peer unavailable"
    );
    match address.ss_family as c_int {
        libc::AF_UNIX => Ok(Transport::Local),
        _ if encrypted => Ok(Transport::Tls),
        libc::AF_INET if length as usize >= std::mem::size_of::<libc::sockaddr_in>() => {
            let address =
                unsafe { *(&address as *const libc::sockaddr_storage).cast::<libc::sockaddr_in>() };
            anyhow::ensure!(
                std::net::Ipv4Addr::from(address.sin_addr.s_addr.to_ne_bytes()).is_loopback(),
                "Printer authentication requires encryption"
            );
            Ok(Transport::Loopback)
        }
        libc::AF_INET6 if length as usize >= std::mem::size_of::<libc::sockaddr_in6>() => {
            let address = unsafe {
                *(&address as *const libc::sockaddr_storage).cast::<libc::sockaddr_in6>()
            };
            let address = std::net::Ipv6Addr::from(address.sin6_addr.s6_addr);
            anyhow::ensure!(
                address.is_loopback() || address.to_ipv4_mapped().is_some_and(|v| v.is_loopback()),
                "Printer authentication requires encryption"
            );
            Ok(Transport::Loopback)
        }
        _ => anyhow::bail!("Printer authentication requires encryption"),
    }
}

/// The actual rejected challenge moves to an empty public CUPS HTTP context.
/// Call only outside a checked action; revalidate job identity before mutation.
pub(super) fn continue_session(
    previous: &Session,
    resource: &CStr,
    challenge: &str,
) -> anyhow::Result<Session> {
    let attempts = previous.authentication.attempts();
    let next = Session::open_protected(Some(previous.authentication.protection()))?;
    next.authentication.continue_after(attempts);
    let challenge = CString::new(challenge)?;
    unsafe {
        (next.api.protocol.set_header)(next.http.as_ptr(), 26, challenge.as_ptr());
    }
    anyhow::ensure!(
        unsafe {
            (next.api.authentication.authenticate)(
                next.http.as_ptr(),
                c"POST".as_ptr(),
                resource.as_ptr(),
            )
        } == 0,
        "Printer authentication was not approved"
    );
    Ok(next)
}
