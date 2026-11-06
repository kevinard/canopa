//! Public HTTP/IPP bindings shared by checked submission and recovery.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct Functions {
    pub(super) write: unsafe extern "C" fn(Http, *const c_char, usize) -> isize,
    pub(super) update: unsafe extern "C" fn(Http) -> c_int,
    pub(super) flush: unsafe extern "C" fn(Http),
    pub(super) header: unsafe extern "C" fn(Http, c_int) -> *const c_char,
    pub(super) set_header: unsafe extern "C" fn(Http, c_int, *const c_char),
    pub(super) new: unsafe extern "C" fn() -> *mut c_void,
    pub(super) read: unsafe extern "C" fn(Http, *mut c_void) -> c_int,
    pub(super) status: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub(super) delete: unsafe extern "C" fn(*mut c_void),
    pub(super) request: unsafe extern "C" fn(c_int) -> *mut c_void,
    pub(super) version: unsafe extern "C" fn(*mut c_void, c_int, c_int) -> c_int,
    pub(super) string: unsafe extern "C" fn(
        *mut c_void,
        c_int,
        c_int,
        *const c_char,
        *const c_char,
        *const c_char,
    ) -> *mut c_void,
    pub(super) strings: unsafe extern "C" fn(
        *mut c_void,
        c_int,
        c_int,
        *const c_char,
        c_int,
        *const c_char,
        *const *const c_char,
    ) -> *mut c_void,
    pub(super) perform: unsafe extern "C" fn(Http, *mut c_void, *const c_char) -> *mut c_void,
    pub(super) find: unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> *mut c_void,
    pub(super) first: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub(super) next: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub(super) name: unsafe extern "C" fn(*mut c_void) -> *const c_char,
    pub(super) boolean:
        unsafe extern "C" fn(*mut c_void, c_int, *const c_char, c_char) -> *mut c_void,
    pub(super) group: unsafe extern "C" fn(*mut c_void) -> c_int,
}
impl Functions {
    pub fn load(library: &Library) -> anyhow::Result<Self> {
        // SAFETY: public CUPS2 http/ipp declarations; Api retains the library.
        unsafe {
            Ok(Self {
                write: *library.get(b"httpWrite2\0")?,
                update: *library.get(b"httpUpdate\0")?,
                flush: *library.get(b"httpFlush\0")?,
                header: *library.get(b"httpGetField\0")?,
                set_header: *library.get(b"httpSetField\0")?,
                new: *library.get(b"ippNew\0")?,
                read: *library.get(b"ippRead\0")?,
                status: *library.get(b"ippGetStatusCode\0")?,
                delete: *library.get(b"ippDelete\0")?,
                request: *library.get(b"ippNewRequest\0")?,
                version: *library.get(b"ippSetVersion\0")?,
                string: *library.get(b"ippAddString\0")?,
                strings: *library.get(b"ippAddStrings\0")?,
                perform: *library.get(b"cupsDoRequest\0")?,
                find: *library.get(b"ippFindAttribute\0")?,
                first: *library.get(b"ippFirstAttribute\0")?,
                next: *library.get(b"ippNextAttribute\0")?,
                name: *library.get(b"ippGetName\0")?,
                boolean: *library.get(b"ippAddBoolean\0")?,
                group: *library.get(b"ippGetGroupTag\0")?,
            })
        }
    }
}
pub(super) struct Message {
    pub(super) raw: *mut c_void,
    delete: unsafe extern "C" fn(*mut c_void),
}
impl Message {
    pub(super) fn new(raw: *mut c_void, functions: Functions) -> anyhow::Result<Self> {
        anyhow::ensure!(!raw.is_null(), "Printer response unavailable");
        Ok(Self {
            raw,
            delete: functions.delete,
        })
    }
    pub(super) fn take(mut self) -> *mut c_void {
        std::mem::replace(&mut self.raw, ptr::null_mut())
    }
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
