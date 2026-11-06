use std::{fs::File, io::Write};

pub(super) struct Keymap {
    pub file: File,
    pub size: u32,
}
impl Keymap {
    pub fn new(text: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !text.is_empty() && text.len() <= 256 * 1024 && !text.contains('\0'),
            "Invalid capture keymap"
        );
        use rustix::fs::{MemfdFlags, SealFlags};
        let mut file = File::from(rustix::fs::memfd_create(
            "canopa-captured-keymap",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )?);
        file.write_all(text.as_bytes())?;
        file.write_all(&[0])?;
        rustix::fs::fcntl_add_seals(
            &file,
            SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE | SealFlags::SEAL,
        )?;
        Ok(Self {
            file,
            size: (text.len() + 1) as u32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::FileExt;
    #[test]
    fn published_keymap_bytes_stay_immutable_and_have_the_protocol_terminator() {
        let mut map = Keymap::new("xkb_keymap {};").unwrap();
        let mut bytes = vec![0; map.size as usize];
        assert_eq!(map.file.read_at(&mut bytes, 0).unwrap(), bytes.len());
        assert_eq!(&bytes, b"xkb_keymap {};\0");
        assert!(map.file.write_all(b"changed").is_err());
        assert!(map.file.set_len(0).is_err());
        assert!(map.file.set_len(u64::from(map.size) + 1).is_err());
    }
    #[test]
    fn invalid_or_oversized_keymaps_are_refused_before_publication() {
        for text in [
            String::new(),
            "xkb\0keymap".into(),
            "x".repeat(256 * 1024 + 1),
        ] {
            assert!(Keymap::new(&text).is_err());
        }
    }
}
