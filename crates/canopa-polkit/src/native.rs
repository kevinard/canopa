//! The native listener and session run on their own GLib main context.
use calloop::channel::Sender;
use canopa_session_ui::secret::Secret;
use std::{
    ffi::{CStr, CString, c_char, c_void},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
static RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug)]
pub enum Event {
    Ready,
    Begin {
        generation: u64,
        action: String,
        message: String,
        uids: Vec<u32>,
        selected: usize,
    },
    Prompt {
        generation: u64,
        serial: u64,
        text: String,
        echo: bool,
    },
    Info {
        generation: u64,
        text: String,
        error: bool,
    },
    Failed {
        generation: u64,
    },
    Closed {
        generation: u64,
    },
    Waiting {
        generation: u64,
        serial: u64,
        selected: usize,
    },
    Fatal,
    Stopping,
    Stopped(i32),
}
#[repr(C)]
struct Message {
    kind: u32,
    generation: u64,
    prompt: u64,
    action: *const c_char,
    message: *const c_char,
    text: *const c_char,
    uids: *const u32,
    count: usize,
    selected: u32,
    echo: u32,
}
unsafe extern "C" {
    fn canopa_polkit_run(
        session: *const c_char,
        timeout: u32,
        callback: unsafe extern "C" fn(*const Message, *mut c_void),
        data: *mut c_void,
    ) -> i32;
    fn canopa_polkit_command(
        generation: u64,
        prompt: u64,
        action: u32,
        index: u32,
        response: *const c_char,
        length: usize,
    );
}
unsafe extern "C" fn receive(message: *const Message, data: *mut c_void) {
    // SAFETY: native callbacks borrow a valid Message and this thread-owned Sender.
    let (m, sender) = unsafe { (&*message, &*data.cast::<Sender<Event>>()) };
    let text = |p: *const c_char| {
        if p.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
        }
    };
    let event = match m.kind {
        0 => Event::Ready,
        1 => {
            if m.count == 0 || m.count > 32 || m.uids.is_null() {
                return;
            }
            Event::Begin {
                generation: m.generation,
                action: text(m.action),
                message: text(m.message),
                uids: unsafe { std::slice::from_raw_parts(m.uids, m.count) }.to_vec(),
                selected: m.selected as usize,
            }
        }
        2 => Event::Prompt {
            generation: m.generation,
            serial: m.prompt,
            text: text(m.text),
            echo: m.echo != 0,
        },
        3 | 4 => Event::Info {
            generation: m.generation,
            text: text(m.text),
            error: m.kind == 4,
        },
        5 => Event::Failed {
            generation: m.generation,
        },
        6 => Event::Closed {
            generation: m.generation,
        },
        7 => Event::Waiting {
            generation: m.generation,
            serial: m.prompt,
            selected: m.selected as usize,
        },
        10 => Event::Stopping,
        _ => Event::Fatal,
    };
    let _ = sender.send(event);
}
pub struct Runtime {
    thread: Option<std::thread::JoinHandle<()>>,
    stopped: std::sync::mpsc::Receiver<()>,
}
impl Runtime {
    pub fn start(session: &str, timeout: u32, sender: Sender<Event>) -> anyhow::Result<Self> {
        let session = CString::new(session)?;
        anyhow::ensure!(
            RUNNING
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok(),
            "an agent runtime is already running"
        );
        let (done, stopped) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let sender = Box::new(sender);
            // SAFETY: C invokes the callback only before run returns; Sender stays owned here.
            let status = unsafe {
                canopa_polkit_run(
                    session.as_ptr(),
                    timeout,
                    receive,
                    (&*sender as *const Sender<Event>).cast_mut().cast(),
                )
            };
            let _ = sender.send(Event::Stopped(status));
            let _ = done.send(());
        });
        Ok(Self {
            thread: Some(thread),
            stopped,
        })
    }
    pub fn command(
        generation: u64,
        serial: u64,
        action: u32,
        index: usize,
        response: Option<&Secret>,
    ) {
        let (ptr, length) = response
            .map(|s| (s.as_c_str().as_ptr(), s.as_bytes().len()))
            .unwrap_or((std::ptr::null(), 0));
        // SAFETY: C copies the bounded response synchronously and erases its copy on dispatch.
        unsafe { canopa_polkit_command(generation, serial, action, index as u32, ptr, length) };
    }
}
impl Drop for Runtime {
    fn drop(&mut self) {
        Self::command(0, 0, 0, 0, None);
        if self.stopped.recv_timeout(Duration::from_secs(2)).is_err() {
            // A blocked native provider must not retain an agent registration or input.
            eprintln!("canopa-polkit: authentication backend did not stop");
            std::process::exit(1);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        RUNNING.store(false, Ordering::Release);
    }
}
