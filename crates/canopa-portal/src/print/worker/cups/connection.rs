//! Keep libcups authentication/reconnect logic, but permit connect only while
//! looking up job identity. A guarded action must use the checked connection.
//! This is a sequencing guard for trusted libcups, not an application sandbox.
use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

#[repr(C)]
#[derive(Default)]
struct Data {
    nr: i32,
    arch: u32,
    instruction: u64,
    args: [u64; 6],
}
#[repr(C)]
#[derive(Default)]
struct Notification {
    id: u64,
    pid: u32,
    flags: u32,
    data: Data,
}
#[repr(C)]
#[derive(Default)]
struct Response {
    id: u64,
    value: i64,
    error: i32,
    flags: u32,
}
#[repr(C)]
#[derive(Default)]
struct Sizes {
    notification: u16,
    response: u16,
    data: u16,
}

const RECV: libc::c_ulong = 0xc0502100;
const SEND: libc::c_ulong = 0xc0182101;
const CONTINUE: u32 = 1;
const USER_NOTIF: u32 = 0x7fc00000;
const ALLOW: u32 = 0x7fff0000;
const ERRNO: u32 = 0x00050000;

fn architecture() -> io::Result<u32> {
    if cfg!(target_arch = "x86_64") {
        Ok(0xc000003e)
    } else if cfg!(target_arch = "aarch64") {
        Ok(0xc00000b7)
    } else if cfg!(target_arch = "riscv64") {
        Ok(0xc00000f3)
    } else {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Printer connection guard architecture is unsupported",
        ))
    }
}

struct State {
    cancelling: AtomicBool,
    denied: AtomicBool,
    healthy: AtomicBool,
}
pub(super) struct Connections {
    state: Arc<State>,
    stop: Option<OwnedFd>,
    supervisor: Option<JoinHandle<io::Result<()>>>,
}

impl Connections {
    pub fn new() -> io::Result<Self> {
        let arch = architecture()?;
        let mut sizes = Sizes::default();
        // SAFETY: GET_NOTIF_SIZES writes exactly this public Linux UAPI layout.
        if unsafe { libc::syscall(libc::SYS_seccomp, 3_u32, 0_u32, &mut sizes as *mut Sizes) } < 0 {
            return Err(io::Error::last_os_error());
        }
        if usize::from(sizes.notification) != size_of::<Notification>()
            || usize::from(sizes.response) != size_of::<Response>()
            || usize::from(sizes.data) != size_of::<Data>()
        {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Printer connection notification ABI is unsupported",
            ));
        }
        let (stop_read, stop_write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC)?;
        let instruction = |code, jt, jf, k| libc::sock_filter { code, jt, jf, k };
        let mut program = vec![
            instruction(0x20, 0, 0, 4), // architecture
            instruction(0x15, 1, 0, arch),
            instruction(0x06, 0, 0, ERRNO | libc::ENOSYS as u32),
            instruction(0x20, 0, 0, 0), // syscall number
        ];
        if cfg!(target_arch = "x86_64") {
            program.extend([
                instruction(0x45, 0, 1, 0x40000000), // reject the x32 ABI
                instruction(0x06, 0, 0, ERRNO | libc::ENOSYS as u32),
            ]);
        }
        program.extend([
            instruction(0x15, 0, 1, libc::SYS_connect as u32),
            instruction(0x06, 0, 0, USER_NOTIF),
            instruction(0x06, 0, 0, ALLOW),
        ]);
        let filter = libc::sock_fprog {
            len: program.len() as u16,
            filter: program.as_mut_ptr(),
        };
        // SAFETY: the guard is installed only in this isolated synchronous CUPS
        // worker. The kernel copies the complete live BPF program during this call.
        if unsafe {
            libc::prctl(
                libc::PR_SET_NO_NEW_PRIVS,
                1 as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
                0 as libc::c_ulong,
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        let raw = unsafe {
            libc::syscall(
                libc::SYS_seccomp,
                1_u32,
                8_u32,
                &filter as *const libc::sock_fprog,
            )
        };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: NEW_LISTENER returned a uniquely owned close-on-exec fd.
        let listener = unsafe { OwnedFd::from_raw_fd(raw as i32) };
        let state = Arc::new(State {
            cancelling: AtomicBool::new(false),
            denied: AtomicBool::new(false),
            healthy: AtomicBool::new(true),
        });
        let observed = state.clone();
        let supervisor = std::thread::Builder::new()
            .name("print-connect".into())
            .spawn(move || {
                let result = supervise(listener, stop_read, &observed, arch);
                observed.healthy.store(false, Ordering::SeqCst);
                result
            })?;
        Ok(Self {
            state,
            stop: Some(stop_write),
            supervisor: Some(supervisor),
        })
    }

    pub fn cancel<T>(&self, action: impl FnOnce() -> T) -> io::Result<(T, bool)> {
        self.checked(action)
    }

    pub fn checked<T>(&self, action: impl FnOnce() -> T) -> io::Result<(T, bool)> {
        if !self.state.healthy.load(Ordering::SeqCst) {
            return Err(io::Error::other("Printer connection guard stopped"));
        }
        self.state.denied.store(false, Ordering::SeqCst);
        self.state.cancelling.store(true, Ordering::SeqCst);
        struct Reset<'a>(&'a AtomicBool);
        impl Drop for Reset<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::SeqCst);
            }
        }
        let reset = Reset(&self.state.cancelling);
        let result = action();
        drop(reset);
        if !self.state.healthy.load(Ordering::SeqCst) {
            return Err(io::Error::other("Printer connection guard stopped"));
        }
        Ok((result, self.state.denied.swap(false, Ordering::SeqCst)))
    }
}

impl Drop for Connections {
    fn drop(&mut self) {
        self.state.cancelling.store(true, Ordering::SeqCst);
        drop(self.stop.take());
        // A notification can disappear between poll and RECV. Wake a possible
        // blocked RECV with an invalid-fd connect, which can never contact any
        // endpoint. The supervisor either denies it or has already closed its
        // listener, in which case the kernel returns ENOSYS. No signal handler
        // or detached thread is needed for shutdown.
        unsafe {
            libc::connect(-1, std::ptr::null(), 0);
        }
        if !matches!(
            self.supervisor
                .take()
                .expect("owned connection supervisor")
                .join(),
            Ok(Ok(()))
        ) {
            eprintln!("level=warn component=print event=connection_supervisor_unconfirmed");
        }
    }
}

fn supervise(listener: OwnedFd, stop: OwnedFd, state: &State, arch: u32) -> io::Result<()> {
    use rustix::event::{PollFd, PollFlags, poll};
    loop {
        let mut descriptors = [
            PollFd::new(&listener, PollFlags::IN),
            PollFd::new(&stop, PollFlags::IN),
        ];
        match poll(&mut descriptors, None) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
        }
        if descriptors[1]
            .revents()
            .intersects(PollFlags::IN | PollFlags::HUP)
        {
            return Ok(());
        }
        let mut notification = Notification::default();
        // SAFETY: the notification is zeroed and the exact kernel ABI sizes were
        // verified before installation. No tracee pointer/arguments are inspected.
        if unsafe {
            libc::ioctl(
                listener.as_raw_fd(),
                RECV,
                &mut notification as *mut Notification,
            )
        } < 0
        {
            let error = io::Error::last_os_error();
            if matches!(error.raw_os_error(), Some(libc::EINTR | libc::ENOENT)) {
                continue;
            }
            return Err(error);
        }
        let block = state.cancelling.load(Ordering::SeqCst);
        let expected =
            notification.data.arch == arch && notification.data.nr == libc::SYS_connect as i32;
        let response = Response {
            id: notification.id,
            error: if !expected {
                -libc::ENOSYS
            } else if block {
                -libc::EPERM
            } else {
                0
            },
            flags: if expected && !block { CONTINUE } else { 0 },
            ..Response::default()
        };
        if block && expected {
            state.denied.store(true, Ordering::SeqCst);
        }
        // CONTINUE only preserves ordinary lookup behavior; it does not authorize
        // an address or inspect mutable sockaddr memory. During cancellation no
        // connect is continued, irrespective of arguments.
        loop {
            if unsafe { libc::ioctl(listener.as_raw_fd(), SEND, &response as *const Response) } >= 0
            {
                break;
            }
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            if error.raw_os_error() == Some(libc::ENOENT) {
                break;
            }
            return Err(error);
        }
    }
}
