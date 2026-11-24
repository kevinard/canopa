//! Small in-memory window images from Canopa's Niri fork.
use std::time::Duration;
use zbus::{Connection, fdo::DBusProxy, names::OwnedUniqueName};

const SERVICE: &str = "org.canopa.Niri.WindowPreview1";
const OBJECT: &str = "/org/canopa/Niri/WindowPreview1";
const TIMEOUT: Duration = Duration::from_secs(2);

pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}

pub struct Client {
    connection: Connection,
    owner: OwnedUniqueName,
}

impl Client {
    /// Resolve the service once, optionally binding it to the process that
    /// supplied the consumer's window IDs. Requests use that unique bus name.
    pub async fn connect(connection: Connection, expected_pid: Option<u32>) -> Option<Self> {
        let owner = tokio::time::timeout(TIMEOUT, async {
            let bus = DBusProxy::new(&connection).await.ok()?;
            let owner = bus.get_name_owner(SERVICE.try_into().ok()?).await.ok()?;
            if let Some(pid) = expected_pid
                && bus
                    .get_connection_unix_process_id(owner.as_str().try_into().ok()?)
                    .await
                    .ok()?
                    != pid
            {
                return None;
            }
            Some(owner)
        })
        .await
        .ok()??;
        Some(Self { connection, owner })
    }

    pub async fn capture(&self, window: u64, side: u32) -> Option<Preview> {
        let side = side.clamp(16, 512);
        let reply = tokio::time::timeout(
            TIMEOUT,
            self.connection.call_method(
                Some(self.owner.as_str()),
                OBJECT,
                Some(SERVICE),
                "Capture",
                &(window, side),
            ),
        )
        .await
        .ok()?
        .ok()?;
        let (width, height, png) = reply.body().deserialize().ok()?;
        validate(width, height, png, side)
    }
}

fn validate(width: u32, height: u32, png: Vec<u8>, side: u32) -> Option<Preview> {
    // Check the declared size and PNG header before handing bytes to the renderer.
    if !(1..=side).contains(&width)
        || !(1..=side).contains(&height)
        || png.len() > (4 * side * side + 64 * 1024) as usize
        || !png.starts_with(b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR")
        || png.get(16..20) != Some(&width.to_be_bytes())
        || png.get(20..24) != Some(&height.to_be_bytes())
    {
        return None;
    }
    Some(Preview { width, height, png })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_must_match_the_bounded_png_header() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend(96u32.to_be_bytes());
        png.extend(72u32.to_be_bytes());
        assert!(validate(96, 72, png.clone(), 192).is_some());
        assert!(validate(96, 71, png.clone(), 192).is_none());
        assert!(validate(96, 72, png, 64).is_none());
    }
}
