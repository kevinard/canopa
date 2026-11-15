use anyhow::{Result, ensure};
use std::{
    ops::{Deref, DerefMut},
    ptr::NonNull,
};
use unicode_segmentation::UnicodeSegmentation;
use zeroize::Zeroize;

/// A private mapping prevents munlock from affecting another allocator allocation.
struct Storage {
    pointer: NonNull<u8>,
    mapped: usize,
    capacity: usize,
    locked: bool,
}
// SAFETY: the mapping is exclusively owned; immutable references cannot mutate it.
unsafe impl Send for Storage {}
unsafe impl Sync for Storage {}
impl Storage {
    fn new(capacity: usize) -> Self {
        // SAFETY: sysconf takes no pointers; mmap allocates a private anonymous range.
        let (pointer, mapped) = unsafe {
            let page = libc::sysconf(libc::_SC_PAGESIZE).max(1) as usize;
            let mapped = capacity.div_ceil(page) * page;
            let pointer = libc::mmap(
                std::ptr::null_mut(),
                mapped,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            );
            assert_ne!(pointer, libc::MAP_FAILED, "cannot allocate secret storage");
            (
                NonNull::new(pointer.cast::<u8>()).expect("secret mapping"),
                mapped,
            )
        };
        // SAFETY: both operations cover the live mapping. Locking is best effort;
        // an unprivileged process can exhaust its RLIMIT_MEMLOCK without losing auth.
        let locked = unsafe {
            libc::madvise(pointer.as_ptr().cast(), mapped, libc::MADV_DONTDUMP);
            libc::mlock(pointer.as_ptr().cast(), mapped) == 0
        };
        Self {
            pointer,
            mapped,
            capacity,
            locked,
        }
    }
}
impl Deref for Storage {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        // SAFETY: initialized anonymous mapping, valid for this owner's lifetime.
        unsafe { std::slice::from_raw_parts(self.pointer.as_ptr(), self.capacity) }
    }
}
impl DerefMut for Storage {
    fn deref_mut(&mut self) -> &mut [u8] {
        // SAFETY: &mut self guarantees exclusive access to the owned mapping.
        unsafe { std::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.capacity) }
    }
}
impl Drop for Storage {
    fn drop(&mut self) {
        self.deref_mut().zeroize();
        // SAFETY: this is the original private mapping; no references escape drop.
        unsafe {
            if self.locked {
                libc::munlock(self.pointer.as_ptr().cast(), self.mapped);
            }
            libc::munmap(self.pointer.as_ptr().cast(), self.mapped);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edit {
    Left(bool),
    Right(bool),
    Home(bool),
    End(bool),
    SelectAll,
    Backspace,
    Delete,
    Clear,
}

/// Bounded, non-reallocating UTF-8 input. Cursor and selection use byte boundaries.
pub struct Secret {
    bytes: Storage,
    len: usize,
    revision: u64,
    cursor: usize,
    anchor: usize,
}
impl Default for Secret {
    fn default() -> Self {
        Self::with_capacity(1024)
    }
}
impl Secret {
    /// Includes the NUL terminator; owned pages are best-effort mlocked.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!((1..=8193).contains(&capacity));
        Self {
            bytes: Storage::new(capacity),
            len: 0,
            revision: 0,
            cursor: 0,
            anchor: 0,
        }
    }
    pub(crate) fn revision(&self) -> (usize, u64) {
        (self.bytes.pointer.as_ptr() as usize, self.revision)
    }
    pub fn memory_locked(&self) -> bool {
        self.bytes.locked
    }
    pub fn selection(&self) -> std::ops::Range<usize> {
        self.cursor.min(self.anchor)..self.cursor.max(self.anchor)
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn anchor(&self) -> usize {
        self.anchor
    }
    pub fn set_selection(&mut self, anchor: usize, cursor: usize) -> bool {
        let boundary = |p| p == self.len || self.text().grapheme_indices(true).any(|(i, _)| i == p);
        if anchor <= self.len && cursor <= self.len && boundary(anchor) && boundary(cursor) {
            self.anchor = anchor;
            self.cursor = cursor;
            true
        } else {
            false
        }
    }
    pub fn push(&mut self, text: &str) {
        let range = self.selection();
        let length = self.len - range.len() + text.len();
        if text.is_empty() || text.chars().any(char::is_control) || length >= self.bytes.len() {
            return;
        }
        self.revision = self.revision.wrapping_add(1);
        self.bytes
            .copy_within(range.end..self.len, range.start + text.len());
        self.bytes[range.start..range.start + text.len()].copy_from_slice(text.as_bytes());
        if length < self.len {
            self.bytes[length..self.len].zeroize();
        }
        self.len = length;
        self.cursor = range.start + text.len();
        // Joining a combining sequence can remove a previous grapheme boundary.
        // Move to the next complete grapheme without allocating secret text.
        self.cursor = self
            .text()
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|i| *i >= self.cursor)
            .unwrap_or(self.len);
        self.anchor = self.cursor;
    }
    fn remove(&mut self, range: std::ops::Range<usize>) {
        self.revision = self.revision.wrapping_add(1);
        let length = self.len - range.len();
        self.bytes.copy_within(range.end..self.len, range.start);
        self.bytes[length..self.len].zeroize();
        self.len = length;
        self.cursor = range.start;
        self.cursor = self
            .text()
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|i| *i >= self.cursor)
            .unwrap_or(self.len);
        self.anchor = self.cursor;
    }
    pub fn pop(&mut self) {
        self.edit(Edit::Backspace);
    }
    pub fn edit(&mut self, edit: Edit) {
        let previous = self
            .text()
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .take_while(|i| *i < self.cursor)
            .last()
            .unwrap_or(0);
        let next = self
            .text()
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|i| *i > self.cursor)
            .unwrap_or(self.len);
        let selected = self.selection();
        let (position, selecting) = match edit {
            Edit::Left(s) => (
                if !s && !selected.is_empty() {
                    selected.start
                } else {
                    previous
                },
                s,
            ),
            Edit::Right(s) => (
                if !s && !selected.is_empty() {
                    selected.end
                } else {
                    next
                },
                s,
            ),
            Edit::Home(s) => (0, s),
            Edit::End(s) => (self.len, s),
            Edit::SelectAll => {
                self.anchor = 0;
                self.cursor = self.len;
                return;
            }
            Edit::Clear => {
                self.clear();
                return;
            }
            Edit::Backspace | Edit::Delete => {
                let range = if !selected.is_empty() {
                    selected
                } else if edit == Edit::Backspace {
                    previous..self.cursor
                } else {
                    self.cursor..next
                };
                self.remove(range);
                return;
            }
        };
        self.cursor = position;
        if !selecting {
            self.anchor = position;
        }
    }
    pub fn clear(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.bytes.deref_mut().zeroize();
        self.len = 0;
        self.cursor = 0;
        self.anchor = 0;
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn text(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).expect("secret UTF-8 invariant")
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
    pub fn as_c_str(&self) -> &std::ffi::CStr {
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
        secret.cursor = length;
        secret.anchor = length;
        Ok(secret)
    }
    pub fn count(&self) -> usize {
        self.text().chars().count()
    }
    pub fn grapheme_count(&self) -> usize {
        self.text().graphemes(true).count()
    }
    /// The grapheme boundaries (byte offsets, never text) and the visible
    /// `start..=end` boundary indices of the viewport of at most `maximum`
    /// graphemes around the cursor. Rendering and pointer hit-testing share it,
    /// so a click always maps to the offset that was drawn there.
    fn window(&self, maximum: usize) -> (Vec<usize>, usize, usize) {
        let boundaries: Vec<_> = self
            .text()
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain(std::iter::once(self.len))
            .collect();
        let cursor = boundaries.partition_point(|i| *i < self.cursor);
        let start = cursor
            .saturating_sub(maximum / 2)
            .min(boundaries.len().saturating_sub(1 + maximum));
        let end = (start + maximum).min(boundaries.len() - 1);
        (boundaries, start, end)
    }
    /// Display-to-storage boundaries for pointer editing of the bounded viewport.
    /// Positions contain no response text and always end on complete graphemes.
    pub(crate) fn display_indices(&self, echo: bool, maximum: usize) -> Vec<(usize, usize)> {
        let (boundaries, start, end) = self.window(maximum);
        (start..=end)
            .map(|i| {
                (
                    if echo {
                        boundaries[i] - boundaries[start]
                    } else {
                        (i - start) * '•'.len_utf8()
                    },
                    boundaries[i],
                )
            })
            .collect()
    }
    /// Bounded display around the cursor. Hidden input exposes only bullets.
    pub fn presentation(
        &self,
        echo: bool,
        maximum: usize,
    ) -> (zeroize::Zeroizing<String>, std::ops::Range<usize>, usize) {
        let (boundaries, start, end) = self.window(maximum);
        let range = boundaries[start]..boundaries[end];
        let map = |position: usize| {
            if echo {
                position.clamp(range.start, range.end) - range.start
            } else {
                (boundaries
                    .partition_point(|i| *i < position)
                    .clamp(start, end)
                    - start)
                    * '•'.len_utf8()
            }
        };
        let selected = self.selection();
        (
            zeroize::Zeroizing::new(if echo {
                self.text()[range.clone()].to_owned()
            } else {
                "•".repeat(end - start)
            }),
            map(selected.start)..map(selected.end),
            map(self.cursor),
        )
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        self.clear();
    }
}

pub fn protect_process() -> Result<()> {
    // SAFETY: scalar arguments and a valid stack-allocated rlimit.
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
        let mut s = Secret::default();
        s.push("aé🔒");
        assert_eq!(s.count(), 3);
        s.pop();
        assert_eq!(s.count(), 2);
        assert!(s.bytes[3..].iter().all(|b| *b == 0));
        s.push("\0");
        s.push(&"x".repeat(2000));
        assert_eq!(s.count(), 2);
        s.clear();
        assert!(s.bytes.iter().all(|b| *b == 0));
    }
    #[test]
    fn selection_insertion_delete_and_graphemes_preserve_terminator_and_erasure() {
        let mut s = Secret::default();
        s.push("ae\u{301}👩‍💻z");
        s.edit(Edit::Home(false));
        s.edit(Edit::Right(false));
        s.edit(Edit::Right(true));
        assert_eq!(&s.text()[s.selection()], "e\u{301}");
        s.push("é");
        assert_eq!(s.text(), "aé👩‍💻z");
        s.edit(Edit::Delete);
        assert_eq!(s.text(), "aéz");
        s.edit(Edit::End(false));
        s.pop();
        assert_eq!(s.text(), "aé");
        assert!(!s.set_selection(2, 3));
        s.edit(Edit::SelectAll);
        s.push("ok");
        assert_eq!(s.text(), "ok");
        assert_eq!(s.as_c_str().to_bytes(), b"ok");
        assert!(s.bytes[2..].iter().all(|b| *b == 0));
        s.edit(Edit::Clear);
        assert_eq!(s.cursor(), 0);
        assert!(s.selection().is_empty());
    }
    #[test]
    fn rejected_replacement_leaves_selection_untouched() {
        let mut s = Secret::with_capacity(5);
        s.push("abcd");
        s.edit(Edit::SelectAll);
        s.push("12345");
        assert_eq!(s.text(), "abcd");
        assert_eq!(s.selection(), 0..4);
        s.push("é");
        assert_eq!(s.text(), "é");
        assert!(s.bytes[2..].iter().all(|b| *b == 0));
        assert_eq!(s.bytes.pointer.as_ptr() as usize % 4096, 0);
    }
}
