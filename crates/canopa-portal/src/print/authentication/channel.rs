use super::{Challenge, Credentials, MAX_EXCHANGES, MAX_FRAME, Secret};
use rustix::event::{PollFd, PollFlags, poll};
use std::{
    cell::{Cell, RefCell},
    fs::File,
    io::{Read, Write},
    os::unix::net::UnixStream,
    rc::Rc,
    time::{Duration, Instant},
};

const DEADLINE: Duration = Duration::from_secs(120);
thread_local! {
    static CURRENT: RefCell<Option<Rc<Channel>>> = const { RefCell::new(None) };
    static INTERACTIVE: Cell<bool> = const { Cell::new(true) };
}
/// Synchronous retirement may reuse approved credentials, but cannot ask for new
/// consent while unwinding a failed submission. The guard stays on this thread.
#[must_use]
pub(crate) struct CacheOnly {
    previous: bool,
    _thread: std::marker::PhantomData<Rc<()>>,
}
impl CacheOnly {
    pub fn enter() -> Self {
        Self {
            previous: INTERACTIVE.with(|value| value.replace(false)),
            _thread: std::marker::PhantomData,
        }
    }
}
impl Drop for CacheOnly {
    fn drop(&mut self) {
        INTERACTIVE.with(|value| value.set(self.previous));
    }
}
pub(crate) fn current() -> Option<Rc<Channel>> {
    CURRENT.with(|value| value.borrow().clone())
}
pub(crate) struct Scope(Option<Rc<Channel>>);
impl Scope {
    pub fn enter(stream: UnixStream, cancel: File) -> anyhow::Result<Self> {
        stream.peer_addr()?;
        stream.set_nonblocking(true)?;
        let channel = Rc::new(Channel {
            stream: RefCell::new(stream),
            cancel,
            next: Cell::new(0),
            refused: Cell::new(false),
            declined: Cell::new(false),
        });
        let previous = CURRENT.with(|value| value.replace(Some(channel)));
        Ok(Self(previous))
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        CURRENT.with(|value| {
            value.replace(self.0.take());
        });
    }
}
pub(crate) struct Channel {
    stream: RefCell<UnixStream>,
    cancel: File,
    next: Cell<u32>,
    refused: Cell<bool>,
    declined: Cell<bool>,
}
impl Channel {
    pub fn declined(&self) -> bool {
        self.declined.get()
    }
    pub fn request(&self, mut challenge: Challenge) -> anyhow::Result<Option<Credentials>> {
        if self.refused.get() {
            return Ok(None);
        }
        let interactive = challenge.interactive && INTERACTIVE.with(Cell::get);
        challenge.interactive = interactive;
        let result = (|| {
            let id = self.next.get() + 1;
            anyhow::ensure!(id <= MAX_EXCHANGES, "Too many authentication requests");
            self.next.set(id);
            challenge.id = id;
            challenge.validate()?;
            let bytes = serde_json::to_vec(&challenge)?;
            anyhow::ensure!(
                bytes.len() <= MAX_FRAME,
                "Authentication challenge exceeds limits"
            );
            let mut stream = self.stream.try_borrow_mut()?;
            let end = Instant::now() + DEADLINE;
            self.transfer(
                &mut stream,
                &mut (bytes.len() as u32).to_be_bytes(),
                true,
                end,
            )?;
            let mut bytes = bytes;
            self.transfer(&mut stream, &mut bytes, true, end)?;
            let mut length = [0; 4];
            self.transfer(&mut stream, &mut length, false, end)?;
            let length = u32::from_be_bytes(length) as usize;
            anyhow::ensure!(
                (6..=MAX_FRAME).contains(&length),
                "Authentication reply exceeds limits"
            );
            let mut frame = Secret::buffer(length);
            self.transfer(&mut stream, frame.bytes_mut(), false, end)?;
            Credentials::decode(&challenge, frame.bytes())
        })();
        // CUPS may ask again for another internal listing/default request even
        // after a callback returned NULL. An interactive refusal or broken
        // channel is final. A cache miss is not user cancellation and must not
        // prevent reuse of another approved scope during a recovery batch.
        if result.is_err() || (interactive && matches!(result, Ok(None))) {
            self.refused.set(true);
        }
        if interactive && matches!(result, Ok(None)) {
            self.declined.set(true);
        }
        result
    }
    fn transfer(
        &self,
        stream: &mut UnixStream,
        mut bytes: &mut [u8],
        write: bool,
        end: Instant,
    ) -> anyhow::Result<()> {
        while !bytes.is_empty() {
            let remaining = end
                .checked_duration_since(Instant::now())
                .ok_or_else(|| anyhow::anyhow!("Printer authentication timed out"))?;
            let mut fds = [
                PollFd::new(&self.cancel, PollFlags::IN),
                PollFd::new(&*stream, if write { PollFlags::OUT } else { PollFlags::IN }),
            ];
            let timeout = rustix::event::Timespec {
                tv_sec: remaining.as_secs() as i64,
                tv_nsec: i64::from(remaining.subsec_nanos()),
            };
            let ready = poll(&mut fds, Some(&timeout))?;
            anyhow::ensure!(
                ready > 0 && fds[0].revents().is_empty(),
                "Printer authentication cancelled"
            );
            let result = if write {
                stream.write(bytes)
            } else {
                stream.read(bytes)
            };
            match result {
                Ok(0) => anyhow::bail!("Printer authentication channel closed"),
                Ok(length) => bytes = &mut bytes[length..],
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => anyhow::bail!("Printer authentication channel failed"),
            }
        }
        Ok(())
    }
}
