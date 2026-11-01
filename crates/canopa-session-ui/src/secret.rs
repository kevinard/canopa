use anyhow::{Result, ensure};
use zeroize::Zeroize;
/// Fixed capacity avoids leaving reallocated password buffers in the allocator.
pub struct Secret {
    bytes: Box<[u8]>,
    len: usize,
}
impl Default for Secret {
    fn default() -> Self {
        Self::with_capacity(1024)
    }
}
impl Secret {
    /// Allocate once; insertion never reallocates sensitive storage. Includes NUL.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!((1..=8193).contains(&capacity));
        Self {
            bytes: vec![0; capacity].into_boxed_slice(),
            len: 0,
        }
    }
    pub fn push(&mut self, text: &str) {
        if !text.chars().any(char::is_control) && self.len + text.len() < self.bytes.len() {
            self.bytes[self.len..self.len + text.len()].copy_from_slice(text.as_bytes());
            self.len += text.len();
        }
    }
    pub fn pop(&mut self) {
        if self.len == 0 {
            return;
        }
        let mut end = self.len - 1;
        while end > 0 && self.bytes[end] & 0xc0 == 0x80 {
            end -= 1;
        }
        self.bytes[end..self.len].zeroize();
        self.len = end;
    }
    pub fn clear(&mut self) {
        self.bytes.zeroize();
        self.len = 0;
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn text(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
    pub fn as_c_str(&self) -> &std::ffi::CStr {
        // Every insertion keeps the unused tail zeroed.
        std::ffi::CStr::from_bytes_with_nul(&self.bytes[..=self.len]).expect("secret terminator")
    }
    pub fn read_from(reader: &mut impl std::io::Read, length: usize) -> Result<Self> {
        ensure!(length < 1024, "authentication response exceeds budget");
        let mut secret = Self::default();
        reader.read_exact(&mut secret.bytes[..length])?;
        ensure!(
            !secret.bytes[..length].contains(&0)
                && std::str::from_utf8(&secret.bytes[..length]).is_ok(),
            "invalid response encoding"
        );
        secret.len = length;
        Ok(secret)
    }
    pub fn count(&self) -> usize {
        std::str::from_utf8(&self.bytes[..self.len])
            .unwrap_or("")
            .chars()
            .count()
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        self.clear();
    }
}
pub fn protect_process() -> Result<()> {
    // SAFETY: these calls use scalar arguments and a valid stack-allocated rlimit.
    unsafe {
        let limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        ensure!(
            libc::setrlimit(libc::RLIMIT_CORE, &limit) == 0,
            "cannot disable core dumps"
        );
        ensure!(
            libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) == 0,
            "cannot protect authentication memory"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn secret_input_is_bounded_utf8_and_erases_deleted_bytes() {
        let mut secret = Secret::default();
        secret.push("aé🔒");
        assert_eq!(secret.count(), 3);
        secret.pop();
        assert_eq!(secret.count(), 2);
        assert!(secret.bytes[3..].iter().all(|b| *b == 0));
        secret.push("\0");
        assert_eq!(secret.count(), 2);
        secret.push(&"x".repeat(2000));
        assert_eq!(secret.count(), 2);
        secret.clear();
        assert!(secret.bytes.iter().all(|b| *b == 0));
    }
}
