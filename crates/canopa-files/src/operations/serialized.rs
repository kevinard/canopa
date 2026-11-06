//! Memoize pure computations by complete input bytes, never filesystem authority.
use super::*;
use sha2::{Digest, Sha256};
use std::{cell::RefCell, collections::VecDeque, sync::Arc};

const LIMIT: usize = 4 * 1024 * 1024;

/// JSON keeps the existing integer-array representation, without dispatching a
/// separate Serialize implementation for every proof byte.
pub(super) fn bytes<const N: usize, S: serde::Serializer>(
    value: &[u8; N],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_bytes(value)
}

// The compact journal writes many short proof arrays. Batch their decimal
// bytes into one write per chunk, keeping Serde's JSON representation intact.
pub(super) struct CompactJournal;

const DECIMAL_BYTES: [([u8; 3], usize); 256] = {
    let mut table = [([0; 3], 0); 256];
    let mut byte = 0;
    while byte < table.len() {
        let digits = [
            b'0' + (byte / 100) as u8,
            b'0' + (byte / 10 % 10) as u8,
            b'0' + (byte % 10) as u8,
        ];
        let start = if byte >= 100 {
            0
        } else if byte >= 10 {
            1
        } else {
            2
        };
        table[byte] = (digits, start);
        byte += 1;
    }
    table
};

impl serde_json::ser::Formatter for CompactJournal {
    fn write_byte_array<W: io::Write + ?Sized>(
        &mut self,
        writer: &mut W,
        value: &[u8],
    ) -> io::Result<()> {
        const CHUNK: usize = 32;
        let mut buffer = [0; CHUNK * 4];
        let mut separator = b'[';
        for chunk in value.chunks(CHUNK) {
            let mut length = 0;
            for &byte in chunk {
                let (digits, start) = DECIMAL_BYTES[usize::from(byte)];
                let digits = &digits[start..];
                buffer[length] = separator;
                length += 1;
                buffer[length..length + digits.len()].copy_from_slice(digits);
                length += digits.len();
                separator = b',';
            }
            writer.write_all(&buffer[..length])?;
        }
        writer.write_all(if value.is_empty() { b"[]" } else { b"]" })
    }
}

/// One previous body per decoder/thread. Changed bytes always run the decoder.
pub(super) struct Last<T> {
    previous: Option<(String, Arc<T>)>,
}
impl<T> Default for Last<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> Last<T> {
    pub const fn new() -> Self {
        Self { previous: None }
    }

    pub fn read(
        &mut self,
        body: &str,
        decode: impl FnOnce(&str) -> io::Result<T>,
    ) -> io::Result<Arc<T>> {
        if let Some((previous, value)) = &self.previous
            && previous.as_bytes() == body.as_bytes()
        {
            return Ok(value.clone());
        }
        let value = Arc::new(decode(body)?);
        self.previous = (body.len() <= LIMIT).then(|| (body.to_owned(), value.clone()));
        Ok(value)
    }
}

struct Hashed {
    bytes: Vec<u8>,
    digest: [u8; 32],
}
thread_local! {
    // A few alternating journal bodies, with a combined byte budget per thread.
    static HASHES: RefCell<VecDeque<Hashed>> = const { RefCell::new(VecDeque::new()) };
}

pub(super) fn digest(bytes: &[u8]) -> [u8; 32] {
    HASHES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(index) = cache.iter().position(|value| value.bytes == bytes) {
            let value = cache.remove(index).unwrap();
            let digest = value.digest;
            cache.push_front(value);
            return digest;
        }
        let digest = Sha256::digest(bytes).into();
        if bytes.len() <= LIMIT {
            while cache.len() >= 4
                || cache.iter().map(|value| value.bytes.len()).sum::<usize>() + bytes.len() > LIMIT
            {
                cache.pop_back();
            }
            cache.push_front(Hashed {
                bytes: bytes.to_vec(),
                digest,
            });
        }
        digest
    })
}

pub(super) fn tagged_digest(tag: &[u8], body: &[u8]) -> [u8; 32] {
    let mut bytes = Vec::with_capacity(tag.len() + body.len());
    bytes.extend_from_slice(tag);
    bytes.extend_from_slice(body);
    digest(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_byte_arrays_keep_the_exact_compact_and_pretty_json_wire_format() {
        #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
        struct Previous {
            boot: [u8; 16],
            object: [u8; 32],
        }
        #[derive(Serialize)]
        struct Current {
            #[serde(serialize_with = "bytes")]
            boot: [u8; 16],
            #[serde(serialize_with = "bytes")]
            object: [u8; 32],
        }
        for base in (0..256).step_by(32) {
            let previous = Previous {
                boot: std::array::from_fn(|index| (base + index) as u8),
                object: std::array::from_fn(|index| (base + index) as u8),
            };
            let current = Current {
                boot: previous.boot,
                object: previous.object,
            };
            let body = serde_json::to_string(&current).unwrap();
            assert_eq!(body, serde_json::to_string(&previous).unwrap());
            assert_eq!(body, store::encode(&current).unwrap());
            assert_eq!(
                serde_json::to_string_pretty(&current).unwrap(),
                serde_json::to_string_pretty(&previous).unwrap()
            );
            assert_eq!(serde_json::from_str::<Previous>(&body).unwrap(), previous);
        }
        struct Array<const N: usize>([u8; N]);
        impl<const N: usize> Serialize for Array<N> {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                bytes(&self.0, serializer)
            }
        }
        let all = Array::<256>(std::array::from_fn(|index| index as u8));
        assert_eq!(
            store::encode(&all).unwrap(),
            serde_json::to_string(&all).unwrap()
        );
        assert_eq!(store::encode(&Array([])).unwrap(), "[]");
    }

    #[test]
    fn changed_or_invalid_same_length_bodies_never_reuse_an_old_decoding() {
        let mut cache = Last::<serde_json::Value>::new();
        let decode = |body: &str| serde_json::from_str(body).map_err(io::Error::other);
        let first = cache.read(r#"{"value":1}"#, decode).unwrap();
        let second = cache.read(r#"{"value":2}"#, decode).unwrap();
        assert_eq!(first["value"], 1);
        assert_eq!(second["value"], 2);
        assert!(cache.read(r#"{"value":?}"#, decode).is_err());
        assert_eq!(cache.read(r#"{"value":1}"#, decode).unwrap()["value"], 1);
    }

    #[test]
    fn changed_bytes_and_cache_churn_preserve_standard_sha256_results() {
        for _ in 0..2 {
            for bytes in [
                b"abc".as_slice(),
                b"abd",
                b"longer",
                b"",
                b"abc",
                b"four",
                b"five",
            ] {
                assert_eq!(digest(bytes), <[u8; 32]>::from(Sha256::digest(bytes)));
                assert_eq!(
                    tagged_digest(b"domain\0", bytes),
                    <[u8; 32]>::from(Sha256::digest([b"domain\0".as_slice(), bytes].concat()))
                );
            }
        }
    }
}
