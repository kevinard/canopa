//! Names of local processes that ask for authentication, for display only.
//! A process chooses its own name, so callers label it as the requester and
//! never treat it as proof of identity.

/// The program name from the command line, or the kernel's short name.
pub fn program_name(pid: u32) -> Option<String> {
    let clean = |name: &str| {
        let name: String = name
            .chars()
            .filter(|c| !c.is_control())
            .take(48)
            .collect::<String>()
            .trim()
            .to_owned();
        (!name.is_empty()).then_some(name)
    };
    std::fs::read(format!("/proc/{pid}/cmdline"))
        .ok()
        .and_then(|line| {
            let first = line.split(|byte| *byte == 0).next()?.to_vec();
            let first = String::from_utf8_lossy(&first).into_owned();
            clean(first.rsplit('/').next()?)
        })
        .or_else(|| {
            std::fs::read_to_string(format!("/proc/{pid}/comm"))
                .ok()
                .and_then(|name| clean(&name))
        })
}

/// The parent process ID, from `/proc/PID/stat`.
pub fn parent(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The command name may contain spaces and parentheses; fields follow the last ')'.
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(1)?.parse().ok()
}

/// This host's name, to tell local requesters from forwarded ones.
pub fn hostname() -> Option<String> {
    let mut buffer = [0u8; 256];
    // SAFETY: the buffer outlives the call and its length is passed.
    if unsafe { libc::gethostname(buffer.as_mut_ptr().cast(), buffer.len()) } != 0 {
        return None;
    }
    let end = buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len());
    std::str::from_utf8(&buffer[..end]).ok().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    #[test]
    fn this_process_is_named_and_has_a_parent() {
        let pid = std::process::id();
        assert!(super::program_name(pid).is_some());
        assert!(super::parent(pid).is_some_and(|parent| parent != pid));
        assert!(super::program_name(u32::MAX).is_none());
    }
}
