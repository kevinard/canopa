use super::{Challenge, Kind, MAX_FRAME};

/// The only byte owner for incoming/outgoing secret frames and credential values.
/// CUPS itself also owns its HTTP authentication copy until that worker exits.
pub(crate) struct Secret(Vec<u8>);
impl Secret {
    pub fn from_string(value: String) -> Self {
        Self(value.into_bytes())
    }
    pub fn copy(value: &[u8]) -> Self {
        let mut bytes = Vec::with_capacity(value.len() + 1);
        bytes.extend_from_slice(value);
        Self(bytes)
    }
    pub fn buffer(length: usize) -> Self {
        Self(vec![0; length])
    }
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
    pub fn c_string(mut self) -> anyhow::Result<Self> {
        anyhow::ensure!(!self.0.contains(&0), "Invalid authentication value");
        // copy() reserves the terminator before any secret bytes are written.
        anyhow::ensure!(
            self.0.capacity() > self.0.len(),
            "Missing secret terminator space"
        );
        self.0.push(0);
        Ok(self)
    }
}
impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        // SAFETY: the initialized owned bytes remain allocated for this call.
        // explicit_bzero cannot be optimized away like an unused normal write.
        unsafe {
            libc::explicit_bzero(self.0.as_mut_ptr().cast(), self.0.len());
        }
    }
}
#[derive(Debug)]
pub(crate) struct Credentials {
    pub values: Vec<Secret>,
}
impl Credentials {
    pub fn validate(&self, challenge: &Challenge) -> anyhow::Result<()> {
        Self::validate_values(&self.values, challenge)
    }
    pub fn validate_values(values: &[Secret], challenge: &Challenge) -> anyhow::Result<()> {
        challenge.validate()?;
        anyhow::ensure!(
            values.len() == challenge.fields().len()
                && values.iter().all(|v| v.bytes().len() <= 1023
                    && !v.bytes().contains(&0)
                    && std::str::from_utf8(v.bytes()).is_ok()),
            "Invalid printer credentials"
        );
        if let Kind::Server { limits, .. } = challenge.kind {
            let username = values[0].bytes();
            let password = values[1].bytes();
            // Legacy CUPS also truncates usernames above 64 bytes. Only the
            // verified worker capability can opt into the larger client bounds.
            anyhow::ensure!(
                !username.is_empty()
                    && username.len() <= limits.username_bytes()
                    && !username.contains(&b':')
                    && !username.iter().any(u8::is_ascii_control)
                    && username.len() + 1 + password.len() <= limits.combined_bytes(),
                "Server credentials exceed supported limits"
            );
        }
        Ok(())
    }
    pub fn copy(&self) -> Self {
        Self {
            values: self
                .values
                .iter()
                .map(|value| Secret::copy(value.bytes()))
                .collect(),
        }
    }
    pub fn encode(challenge: &Challenge, credentials: Option<&Self>) -> anyhow::Result<Secret> {
        challenge.validate()?;
        if let Some(value) = credentials {
            value.validate(challenge)?;
        }
        let length = credentials.map_or(6, |value| {
            7 + value
                .values
                .iter()
                .map(|v| 2 + v.bytes().len())
                .sum::<usize>()
        });
        anyhow::ensure!(length <= MAX_FRAME, "Authentication reply exceeds limits");
        let mut frame = Secret::buffer(length);
        let bytes = frame.bytes_mut();
        bytes[0] = 1;
        bytes[1..5].copy_from_slice(&challenge.id.to_be_bytes());
        if let Some(value) = credentials {
            bytes[5] = 1;
            bytes[6] = value.values.len() as u8;
            let mut offset = 7;
            for value in &value.values {
                bytes[offset..offset + 2]
                    .copy_from_slice(&(value.bytes().len() as u16).to_be_bytes());
                offset += 2;
                bytes[offset..offset + value.bytes().len()].copy_from_slice(value.bytes());
                offset += value.bytes().len();
            }
        }
        Ok(frame)
    }
    pub fn decode(challenge: &Challenge, frame: &[u8]) -> anyhow::Result<Option<Self>> {
        anyhow::ensure!(
            frame.len() >= 6
                && frame.len() <= MAX_FRAME
                && frame[0] == 1
                && u32::from_be_bytes(frame[1..5].try_into()?) == challenge.id,
            "Invalid authentication reply"
        );
        if frame[5] == 0 {
            anyhow::ensure!(frame.len() == 6, "Trailing authentication reply data");
            return Ok(None);
        }
        anyhow::ensure!(
            frame[5] == 1 && frame.len() >= 7 && frame[6] as usize == challenge.fields().len(),
            "Invalid authentication reply fields"
        );
        let mut offset = 7;
        let mut values = Vec::new();
        for _ in 0..frame[6] {
            let length = frame
                .get(offset..offset + 2)
                .ok_or_else(|| anyhow::anyhow!("Incomplete authentication reply"))?;
            let length = u16::from_be_bytes(length.try_into()?) as usize;
            offset += 2;
            let value = frame
                .get(offset..offset + length)
                .ok_or_else(|| anyhow::anyhow!("Incomplete authentication value"))?;
            offset += length;
            values.push(Secret::copy(value));
        }
        anyhow::ensure!(offset == frame.len(), "Trailing authentication reply data");
        let result = Self { values };
        result.validate(challenge)?;
        Ok(Some(result))
    }
}
