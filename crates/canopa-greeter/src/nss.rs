//! NSS runs only in a fresh helper: enumeration may block and has process-global state.
use crate::accounts::{Account, merge};
use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::HashSet,
    ffi::CStr,
    fs::OpenOptions,
    io::Read,
    os::unix::fs::OpenOptionsExt,
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
};
use zeroize::{Zeroize, Zeroizing};

const MAX_RECORDS: usize = 4096;
const MAX_USERS: usize = 128;
const MAX_BYTES: usize = 65536;

#[derive(Clone, Debug, Default, clap::Args)]
pub struct Policy {
    #[arg(long)]
    pub uid_min: Option<u32>,
    #[arg(long)]
    pub uid_max: Option<u32>,
    #[arg(long)]
    pub include_nologin: bool,
}
impl Policy {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.uid_max != Some(u32::MAX) && self.uid_min != Some(u32::MAX),
            "invalid NSS UID bound"
        );
        ensure!(
            self.uid_min
                .zip(self.uid_max)
                .is_none_or(|(min, max)| min <= max),
            "NSS UID minimum exceeds maximum"
        );
        Ok(())
    }
    fn range(&self, defaults: &str) -> Result<(u32, u32)> {
        let (mut min, mut max) = (1000, 60000);
        for line in defaults.lines() {
            let mut fields = line
                .split('#')
                .next()
                .unwrap_or_default()
                .split_whitespace();
            if let (Some(key), Some(value)) = (fields.next(), fields.next())
                && let Ok(value) = value.parse::<u32>()
                && value < u32::MAX
            {
                match key {
                    "UID_MIN" => min = value,
                    "UID_MAX" => max = value,
                    _ => {}
                }
            }
        }
        let range = (self.uid_min.unwrap_or(min), self.uid_max.unwrap_or(max));
        ensure!(range.0 <= range.1, "invalid effective NSS UID range");
        Ok(range)
    }
}

fn login_defs() -> String {
    let Ok(file) = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open("/etc/login.defs")
    else {
        return String::new();
    };
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return String::new();
    }
    let mut text = String::new();
    if file
        .take(MAX_BYTES as u64 + 1)
        .read_to_string(&mut text)
        .is_err()
        || text.len() > MAX_BYTES
    {
        return String::new();
    }
    text
}

/// Invoked before configuration, Wayland, authentication or other worker threads start.
pub fn helper(policy: &Policy) -> Result<Vec<Account>> {
    policy.validate()?;
    canopa_session_ui::secret::protect_process()?;
    let (min, max) = policy.range(&login_defs())?;
    let mut buffer = Zeroizing::new(vec![0u8; MAX_BYTES]);
    let mut result = Vec::new();
    let mut names = HashSet::new();
    let mut uids = HashSet::new();
    struct Enumeration;
    impl Drop for Enumeration {
        fn drop(&mut self) {
            unsafe { libc::endpwent() };
        }
    }
    // SAFETY: the isolated helper is single-threaded; NSS owns no state in the frontend.
    unsafe { libc::setpwent() };
    let _enumeration = Enumeration;
    for _ in 0..MAX_RECORDS {
        let mut record = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut found = std::ptr::null_mut();
        // SAFETY: passwd and its fixed backing buffer remain valid through each conversion.
        let status = unsafe {
            libc::getpwent_r(
                record.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut found,
            )
        };
        if status == libc::ENOENT || (status == 0 && found.is_null()) {
            return Ok(merge(&[], result));
        }
        ensure!(
            status == 0 && !found.is_null(),
            "NSS enumeration failed or record exceeds budget"
        );
        let record = unsafe { record.assume_init() };
        if record.pw_uid >= min && record.pw_uid <= max && record.pw_uid != 0 {
            // SAFETY: successful getpwent_r supplies terminated fields valid until the next call.
            let field = |ptr: *const libc::c_char| -> &[u8] {
                if ptr.is_null() {
                    b""
                } else {
                    unsafe { CStr::from_ptr(ptr) }.to_bytes()
                }
            };
            let shell = field(record.pw_shell);
            let nologin = shell
                .rsplit(|b| *b == b'/')
                .next()
                .is_some_and(|s| s == b"nologin" || s == b"false");
            if (!nologin || policy.include_nologin)
                && shell.len() <= 4096
                && let Ok(username) = std::str::from_utf8(field(record.pw_name))
            {
                let name = field(record.pw_gecos)
                    .split(|b| *b == b',')
                    .next()
                    .unwrap_or_default();
                let real_name = if name.len() <= 128 {
                    std::str::from_utf8(name).unwrap_or_default()
                } else {
                    ""
                };
                if let Some(account) = Account::discovered(
                    username.into(),
                    real_name.into(),
                    u64::from(record.pw_uid),
                    false,
                ) && names.insert(account.username.clone())
                    && uids.insert(account.uid)
                {
                    result.push(account);
                }
            }
        }
        // Do not retain unused NSS fields (including passwd/home/GECOS details).
        buffer.as_mut_slice().zeroize();
        if result.len() == MAX_USERS {
            return Ok(merge(&[], result));
        }
    }
    bail!("NSS enumeration exceeds record budget")
}

/// Frontend-side, cancellable process boundary. At most one helper per discovery.
pub fn discover(policy: &Policy, cancelled: &AtomicBool) -> Result<Vec<Account>> {
    policy.validate()?;
    if cancelled.load(Ordering::Acquire) {
        bail!("NSS discovery cancelled");
    }
    let mut command = Command::new(canopa_file::executable::current()?);
    command.arg("internal-nss-users");
    if let Some(min) = policy.uid_min {
        command.arg("--uid-min").arg(min.to_string());
    }
    if let Some(max) = policy.uid_max {
        command.arg("--uid-max").arg(max.to_string());
    }
    if policy.include_nologin {
        command.arg("--include-nologin");
    }
    let bytes = crate::helper::run(command, &|| cancelled.load(Ordering::Acquire), MAX_BYTES)?;
    let result: Vec<Account> =
        serde_json::from_slice(&bytes).context("invalid NSS helper response")?;
    ensure!(result.len() <= MAX_USERS, "NSS helper user budget exceeded");
    let mut names = HashSet::new();
    let mut uids = HashSet::new();
    for account in &result {
        let uid = account.uid.context("missing NSS UID")?;
        ensure!(
            Account::discovered(
                account.username.clone(),
                account.display_name.clone(),
                uid,
                false
            )
            .as_ref()
                == Some(account)
                && names.insert(&account.username)
                && uids.insert(uid),
            "invalid NSS account response"
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uid_policy_uses_login_defs_and_explicit_bounds() {
        let defaults = "# UID_MIN 9\nUID_MIN 500 # humans\nUID_MAX 90000\nSYS_UID_MIN 100\n";
        assert_eq!(Policy::default().range(defaults).unwrap(), (500, 90000));
        assert_eq!(Policy::default().range("").unwrap(), (1000, 60000));
        assert_eq!(
            Policy {
                uid_min: Some(2000),
                uid_max: Some(300000),
                include_nologin: false
            }
            .range(defaults)
            .unwrap(),
            (2000, 300000)
        );
        assert!(
            Policy {
                uid_max: Some(10),
                ..Policy::default()
            }
            .range(defaults)
            .is_err()
        );
    }
}
