use super::*;

/// PID reuse and reboot do not make a former publisher active again.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Process {
    pid: u32,
    start: u64,
    boot: String,
}
impl Process {
    #[cfg(test)]
    pub fn fixture_retire(&mut self) {
        self.boot.push('x');
    }
    pub fn read(pid: u32) -> io::Result<Self> {
        if pid == 0 {
            return Err(invalid());
        }
        let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
        Ok(Self {
            pid,
            start: start(&stat)?,
            boot,
        })
    }
    pub fn alive(&self) -> io::Result<bool> {
        let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
        if boot != self.boot {
            return Ok(false);
        }
        match std::fs::read_to_string(format!("/proc/{}/stat", self.pid)) {
            Ok(stat) => {
                let state = stat
                    .rsplit_once(')')
                    .and_then(|(_, rest)| rest.split_whitespace().next());
                if start(&stat)? != self.start {
                    return Ok(false);
                }
                if !matches!(state, Some("Z" | "X")) {
                    return Ok(true);
                }
                // A zombie leader can still have live threads. Only a process
                // pidfd (without PIDFD_THREAD) proves the entire group exited.
                let pid =
                    rustix::process::Pid::from_raw(i32::try_from(self.pid).map_err(|_| invalid())?)
                        .ok_or_else(invalid)?;
                let fd =
                    match rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()) {
                        Ok(fd) => fd,
                        Err(rustix::io::Errno::SRCH) => return Ok(false),
                        Err(error) => return Err(error.into()),
                    };
                let mut descriptors = [rustix::event::PollFd::new(
                    &fd,
                    rustix::event::PollFlags::IN,
                )];
                let zero = rustix::event::Timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                };
                rustix::event::poll(&mut descriptors, Some(&zero))?;
                let events = descriptors[0].revents();
                if events.intersects(rustix::event::PollFlags::IN | rustix::event::PollFlags::HUP) {
                    Ok(false)
                } else if events.is_empty() {
                    Ok(true)
                } else {
                    Err(io::Error::other("Worker lifetime is unconfirmed"))
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }
}
fn start(stat: &str) -> io::Result<u64> {
    // comm may contain spaces and ')' characters; the final ')' terminates it.
    stat.rsplit_once(')')
        .and_then(|(_, rest)| rest.split_whitespace().nth(19))
        .and_then(|value| value.parse().ok())
        .ok_or_else(invalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn process_identity_rejects_pid_reuse_and_different_boots() {
        let process = Process::read(std::process::id()).unwrap();
        assert!(process.alive().unwrap());
        let mut reused = process.clone();
        reused.start += 1;
        assert!(!reused.alive().unwrap());
        let mut rebooted = process;
        rebooted.boot.push('x');
        assert!(!rebooted.alive().unwrap());
    }
    #[test]
    fn process_names_cannot_shift_start_time_parsing() {
        let fields = (3..=22)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            start(&format!("100 (a tricky ) process) {fields}")).unwrap(),
            22
        );
        assert!(start("invalid").is_err());
    }

    #[test]
    fn exited_unreaped_workers_are_not_busy_recovery_owners() {
        let mut child = std::process::Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .spawn()
            .unwrap();
        let identity = Process::read(child.id()).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let exited = loop {
            if !identity.alive().unwrap() {
                break true;
            }
            if std::time::Instant::now() >= deadline {
                break false;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        child.wait().unwrap();
        assert!(exited, "a zombie must not prevent printer restart cleanup");
    }
}
