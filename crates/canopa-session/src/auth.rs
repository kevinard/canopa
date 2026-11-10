//! Small Linux-PAM boundary. Only successful auth AND account checks permit unlock.
use anyhow::{Context, Result, ensure};
use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    ptr,
};
use zeroize::{Zeroize, Zeroizing};

pub use canopa_session_ui::secret::{Secret, protect_process};
pub fn username() -> Result<String> {
    // Resolve the real uid, never the caller-controlled USER/LOGNAME variables.
    let mut buffer = vec![0u8; 65536];
    // SAFETY: zero is a valid initialization for passwd; pointers are only read after success.
    unsafe {
        let mut passwd: libc::passwd = std::mem::zeroed();
        let mut result = ptr::null_mut();
        ensure!(
            libc::getpwuid_r(
                libc::getuid(),
                &mut passwd,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result
            ) == 0
                && !result.is_null(),
            "cannot resolve current user"
        );
        Ok(CStr::from_ptr(passwd.pw_name).to_str()?.to_owned())
    }
}
#[repr(C)]
struct Message {
    style: c_int,
    text: *const c_char,
}
#[repr(C)]
struct Response {
    text: *mut c_char,
    code: c_int,
}
#[repr(C)]
struct Conversation {
    callback:
        unsafe extern "C" fn(c_int, *mut *const Message, *mut *mut Response, *mut c_void) -> c_int,
    data: *mut c_void,
}
#[link(name = "pam")]
unsafe extern "C" {
    fn pam_start(
        service: *const c_char,
        user: *const c_char,
        conversation: *const Conversation,
        handle: *mut *mut c_void,
    ) -> c_int;
    fn pam_authenticate(handle: *mut c_void, flags: c_int) -> c_int;
    fn pam_acct_mgmt(handle: *mut c_void, flags: c_int) -> c_int;
    fn pam_chauthtok(handle: *mut c_void, flags: c_int) -> c_int;
    fn pam_end(handle: *mut c_void, status: c_int) -> c_int;
}
/// A module-provided authentication event. It never contains a response or secret.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Event {
    Prompt {
        id: u64,
        text: String,
        echo: bool,
    },
    Message {
        text: String,
        error: bool,
    },
    /// Authentication passed, but PAM requires an explicit password change.
    PasswordExpired {
        id: u64,
    },
    Complete {
        accepted: bool,
    },
}

struct Credentials {
    channel: std::os::unix::net::UnixStream,
    sequence: u64,
}
const CONV_ERROR: c_int = 19;
const MAX_MESSAGE: usize = 4096;
fn message_text(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    // PAM owns a NUL-terminated message. Bound the scan before making Rust text.
    unsafe {
        let length = libc::strnlen(text, MAX_MESSAGE);
        String::from_utf8_lossy(std::slice::from_raw_parts(text.cast::<u8>(), length))
            .chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .take(1024)
            .collect()
    }
}
fn send_event(channel: &mut std::os::unix::net::UnixStream, event: &Event) -> std::io::Result<()> {
    use std::io::Write;
    let bytes = serde_json::to_vec(event)?;
    channel.write_all(&(bytes.len() as u32).to_le_bytes())?;
    channel.write_all(&bytes)
}
pub fn read_event(channel: &mut std::os::unix::net::UnixStream) -> Result<Event> {
    use std::io::Read;
    let mut header = [0; 4];
    channel.read_exact(&mut header)?;
    let length = u32::from_le_bytes(header) as usize;
    ensure!(length <= 16384, "authentication event exceeds budget");
    let mut bytes = vec![0; length];
    channel.read_exact(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}
fn read_answer(channel: &mut std::os::unix::net::UnixStream, expected: u64) -> Result<Secret> {
    use std::io::Read;
    let mut header = [0; 12];
    channel.read_exact(&mut header)?;
    let id = u64::from_le_bytes(header[..8].try_into()?);
    let length = u32::from_le_bytes(header[8..].try_into()?) as usize;
    ensure!(
        id == expected && length < 1024,
        "invalid authentication response"
    );
    Secret::read_from(channel, length)
}
unsafe extern "C" fn conversation(
    count: c_int,
    messages: *mut *const Message,
    replies: *mut *mut Response,
    data: *mut c_void,
) -> c_int {
    if count <= 0 || count > 32 || messages.is_null() || replies.is_null() || data.is_null() {
        return CONV_ERROR;
    }
    // SAFETY: PAM supplies count valid message pointers and our live worker context.
    // calloc/strdup ownership transfers to PAM only on full conversation success.
    unsafe {
        *replies = ptr::null_mut();
        let credentials = &mut *data.cast::<Credentials>();
        let responses =
            libc::calloc(count as usize, std::mem::size_of::<Response>()).cast::<Response>();
        if responses.is_null() {
            return CONV_ERROR;
        }
        for i in 0..count as usize {
            let message = *messages.add(i);
            if message.is_null() {
                free_responses(responses, count as usize);
                return CONV_ERROR;
            }
            let text = message_text((*message).text);
            let result = match (*message).style {
                1 | 2 => {
                    credentials.sequence += 1;
                    let id = credentials.sequence;
                    send_event(
                        &mut credentials.channel,
                        &Event::Prompt {
                            id,
                            text,
                            echo: (*message).style == 2,
                        },
                    )
                    .map_err(anyhow::Error::from)
                    .and_then(|()| read_answer(&mut credentials.channel, id))
                    .and_then(|answer| {
                        let response = libc::strdup(answer.as_c_str().as_ptr());
                        ensure!(!response.is_null(), "PAM response allocation failed");
                        (*responses.add(i)).text = response;
                        Ok(())
                    })
                }
                3 | 4 => send_event(
                    &mut credentials.channel,
                    &Event::Message {
                        text,
                        error: (*message).style == 3,
                    },
                )
                .map_err(Into::into),
                _ => Err(anyhow::anyhow!("unsupported PAM message style")),
            };
            if result.is_err() {
                free_responses(responses, count as usize);
                return CONV_ERROR;
            }
        }
        *replies = responses;
        0
    }
}
unsafe fn free_responses(responses: *mut Response, count: usize) {
    // SAFETY: non-null response strings came from strdup, the array from calloc.
    unsafe {
        for i in 0..count {
            let text = (*responses.add(i)).text;
            if !text.is_null() {
                let len = libc::strlen(text);
                std::slice::from_raw_parts_mut(text.cast::<u8>(), len).zeroize();
                libc::free(text.cast());
            }
        }
        libc::free(responses.cast());
    }
}

/// Internal worker entry point. Its sole peer is the parent's private inherited socket.
pub fn worker(fd: i32) -> Result<()> {
    use std::os::fd::{FromRawFd, OwnedFd};
    ensure!(fd > 2, "invalid authentication channel");
    ensure!(
        unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } >= 0,
        "cannot protect authentication descriptor"
    );
    protect_process()?;
    let user = CString::new(username()?)?;
    // SAFETY: the parent explicitly passes ownership of this inherited descriptor.
    let channel = std::os::unix::net::UnixStream::from(unsafe { OwnedFd::from_raw_fd(fd) });
    let mut credentials = Credentials {
        channel,
        sequence: 0,
    };
    let conv = Conversation {
        callback: conversation,
        data: (&mut credentials as *mut Credentials).cast(),
    };
    let mut handle = ptr::null_mut();
    // SAFETY: CString/context arguments outlive the whole transaction. All successful
    // pam_start calls end with pam_end before success is sent to the UI.
    let accepted = unsafe {
        let mut status = pam_start(c"canopa-lock".as_ptr(), user.as_ptr(), &conv, &mut handle);
        if status == 0 && !handle.is_null() {
            status = pam_authenticate(handle, 1); // PAM_DISALLOW_NULL_AUTHTOK
            let mut expired = false;
            if status == 0 {
                status = pam_acct_mgmt(handle, 1);
                expired = status == 12; // PAM_NEW_AUTHTOK_REQD
            }
            if expired {
                // PAM_NEW_AUTHTOK_REQD: keep this transaction and its authenticated
                // identity, but never start a password change without consent.
                credentials.sequence += 1;
                let id = credentials.sequence;
                let consent = send_event(&mut credentials.channel, &Event::PasswordExpired { id })
                    .map_err(anyhow::Error::from)
                    .and_then(|()| read_answer(&mut credentials.channel, id));
                status = if consent.is_ok_and(|answer| answer.is_empty()) {
                    pam_chauthtok(handle, 0x20) // PAM_CHANGE_EXPIRED_AUTHTOK
                } else {
                    CONV_ERROR
                };
                if status == 0 {
                    status = pam_acct_mgmt(handle, 1);
                }
            }
            let accepted = status == 0;
            let ended = pam_end(handle, status);
            accepted && ended == 0
        } else {
            eprintln!("PAM initialization failed with status {status}");
            false
        }
    };
    send_event(&mut credentials.channel, &Event::Complete { accepted })?;
    Ok(())
}

/// Exactly one isolated PAM transaction. Dropping it kills its process group and
/// closes the conversation; no blocked C module or stale response survives retry.
pub struct Worker {
    child: std::process::Child,
    channel: std::os::unix::net::UnixStream,
    pub started: std::time::Instant,
}
impl Worker {
    pub fn start(mut notify: impl FnMut(Event) + Send + 'static) -> Result<Self> {
        use std::os::{fd::AsRawFd, unix::process::CommandExt};
        let (parent, child) =
            std::os::unix::net::UnixStream::pair().context("create authentication channel")?;
        let fd = child.as_raw_fd();
        let parent_pid = std::process::id() as libc::pid_t;
        let executable =
            canopa_file::executable::current().context("resolve authentication executable")?;
        let mut command = std::process::Command::new(&executable);
        command
            .args(["internal-auth", "--fd", &fd.to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .process_group(0);
        // SAFETY: only async-signal-safe scalar syscalls run between fork and exec.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(fd, libc::F_SETFD, 0) < 0
                    || libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) < 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() != parent_pid {
                    return Err(std::io::Error::other("locker exited"));
                }
                Ok(())
            });
        }
        let mut reader = parent
            .try_clone()
            .context("duplicate authentication channel")?;
        let worker = command
            .spawn()
            .with_context(|| format!("execute authentication worker {}", executable.display()))?;
        drop(child);
        std::thread::spawn(move || {
            loop {
                match read_event(&mut reader) {
                    Ok(event) => {
                        let complete = matches!(event, Event::Complete { .. });
                        notify(event);
                        if complete {
                            break;
                        }
                    }
                    Err(_) => {
                        notify(Event::Complete { accepted: false });
                        break;
                    }
                }
            }
        });
        Ok(Self {
            child: worker,
            channel: parent,
            started: std::time::Instant::now(),
        })
    }
    pub fn respond(&mut self, id: u64, answer: Secret) -> Result<()> {
        use std::io::Write;
        self.channel
            .set_write_timeout(Some(std::time::Duration::from_millis(250)))?;
        self.channel.write_all(&id.to_le_bytes())?;
        self.channel
            .write_all(&(answer.as_bytes().len() as u32).to_le_bytes())?;
        self.channel.write_all(answer.as_bytes())?;
        Ok(())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.channel.shutdown(std::net::Shutdown::Both);
        // SAFETY: this worker has not been waited/reaped anywhere else, so its
        // dedicated process-group id cannot have been recycled. Kill module
        // helpers as well, even if the worker has already finished its transaction.
        unsafe {
            libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL);
        }
        let _ = self.child.wait();
    }
}

/// Clear toolkit-owned key text immediately after copying it to fixed secret memory.
pub fn take_text(text: Option<String>) -> Zeroizing<String> {
    Zeroizing::new(text.unwrap_or_default())
}
pub fn preflight() -> Result<String> {
    std::fs::metadata("/etc/pam.d/canopa-lock")
        .context("install the canopa-lock PAM service before locking")?;
    protect_process()?;
    username()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn successive_challenges_receive_fresh_responses_and_preserve_styles() {
        let (mut ui, pam) = std::os::unix::net::UnixStream::pair().unwrap();
        let conversation = std::thread::spawn(move || {
            let mut credentials = Credentials {
                channel: pam,
                sequence: 0,
            };
            let first = Message {
                style: 1,
                text: c"Password:".as_ptr(),
            };
            let second = Message {
                style: 2,
                text: c"One-time code:".as_ptr(),
            };
            let mut messages = [&first as *const _, &second as *const _];
            let mut replies = ptr::null_mut();
            let result = unsafe {
                conversation(
                    2,
                    messages.as_mut_ptr(),
                    &mut replies,
                    (&mut credentials as *mut Credentials).cast(),
                )
            };
            assert_eq!(result, 0);
            unsafe {
                assert_eq!(CStr::from_ptr((*replies).text).to_bytes(), b"password");
                assert_eq!(CStr::from_ptr((*replies.add(1)).text).to_bytes(), b"123456");
                free_responses(replies, 2);
            }
        });
        use std::io::Write;
        for (expected, answer) in [(1, "password"), (2, "123456")] {
            let Event::Prompt { id, echo, .. } = read_event(&mut ui).unwrap() else {
                panic!()
            };
            assert_eq!(id, expected);
            assert_eq!(echo, expected == 2);
            ui.write_all(&id.to_le_bytes()).unwrap();
            ui.write_all(&(answer.len() as u32).to_le_bytes()).unwrap();
            ui.write_all(answer.as_bytes()).unwrap();
        }
        conversation.join().unwrap();
    }
    #[test]
    fn stale_or_oversized_answers_are_rejected() {
        use std::io::Write;
        for (id, len) in [(9u64, 3u32), (1, 1024)] {
            let (mut ui, mut pam) = std::os::unix::net::UnixStream::pair().unwrap();
            ui.write_all(&id.to_le_bytes()).unwrap();
            ui.write_all(&len.to_le_bytes()).unwrap();
            assert!(read_answer(&mut pam, 1).is_err());
        }
    }
}
