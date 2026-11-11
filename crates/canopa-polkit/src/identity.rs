//! Passive display metadata for an already permitted numeric identity.
//! NSS and image decoding run in a cancellable helper, never in the agent loop.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub uid: u32,
    pub username: String,
    pub name: String,
    pub rgba: Option<Vec<u8>>,
}
impl Identity {
    pub fn label(&self) -> String {
        let name: String = self.name.chars().take(24).collect();
        if self.name.is_empty() || self.name == self.username {
            fl!(
                "polkit-identity",
                username = self.username.as_str(),
                uid = self.uid.to_string()
            )
        } else {
            fl!(
                "polkit-identity-named",
                name = name,
                username = self.username.as_str(),
                uid = self.uid.to_string()
            )
        }
    }
}
fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 128 && !name.chars().any(char::is_control)
}
fn valid_user(name: &str) -> bool {
    valid_name(name)
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-.$".contains(&c))
}
pub fn helper(uid: u32) -> Result<()> {
    use std::{ffi::CStr, io::Write};
    canopa_session_ui::secret::protect_process()?;
    let mut buffer = vec![0; 65536];
    let mut record: libc::passwd = unsafe { std::mem::zeroed() };
    let mut found = std::ptr::null_mut();
    // NSS may block on a network directory; only the deadline-owned child calls it.
    let status = unsafe {
        libc::getpwuid_r(
            uid,
            &mut record,
            buffer.as_mut_ptr(),
            buffer.len(),
            &mut found,
        )
    };
    let mut identity = Identity {
        uid,
        username: String::new(),
        name: String::new(),
        rgba: None,
    };
    if status == 0 && !found.is_null() {
        identity.username = unsafe { CStr::from_ptr(record.pw_name) }.to_str()?.into();
        if !record.pw_gecos.is_null() {
            identity.name = unsafe { CStr::from_ptr(record.pw_gecos) }
                .to_str()?
                .split(',')
                .next()
                .unwrap_or("")
                .into();
        }
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let service = runtime.block_on(async {
        tokio::time::timeout(std::time::Duration::from_millis(600), accounts(uid)).await
    });
    let icon = if let Ok(Ok((username, name, icon))) = service {
        identity.username = username;
        identity.name = name;
        icon
    } else {
        None
    };
    ensure!(
        valid_user(&identity.username),
        "identity metadata unavailable"
    );
    if !valid_name(&identity.name) {
        identity.name.clear();
    }
    let fallback = PathBuf::from("/var/lib/AccountsService/icons").join(&identity.username);
    let paths: Vec<_> = icon.into_iter().chain(std::iter::once(fallback)).collect();
    identity.rgba = thumbnail(&paths);
    serde_json::to_writer(std::io::stdout().lock(), &identity)?;
    std::io::stdout().flush()?;
    Ok(())
}
async fn accounts(uid: u32) -> Result<(String, String, Option<PathBuf>)> {
    use zbus::{Connection, Proxy, zvariant::OwnedObjectPath};
    let connection = Connection::system().await?;
    let root = Proxy::new(
        &connection,
        "org.freedesktop.Accounts",
        "/org/freedesktop/Accounts",
        "org.freedesktop.Accounts",
    )
    .await?;
    let path: OwnedObjectPath = root.call("FindUserById", &(i64::from(uid))).await?;
    let user: Proxy<'_> = zbus::proxy::Builder::new(&connection)
        .destination("org.freedesktop.Accounts")?
        .path(path)?
        .interface("org.freedesktop.Accounts.User")?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()
        .await?;
    let actual: u64 = user.get_property("Uid").await?;
    ensure!(actual == u64::from(uid), "identity metadata mismatch");
    let name: String = user.get_property("RealName").await.unwrap_or_default();
    let username: String = user.get_property("UserName").await?;
    let icon: String = user.get_property("IconFile").await.unwrap_or_default();
    let path = PathBuf::from(icon);
    let icon = (path.is_absolute()
        && path
            .to_str()
            .is_some_and(|s| s.len() <= 4096 && !s.chars().any(char::is_control)))
    .then_some(path);
    Ok((username, name, icon))
}
fn thumbnail(paths: &[PathBuf]) -> Option<Vec<u8>> {
    use canopa_assets::{
        DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
    };
    let mut batch = RasterBatch::new(
        RasterPolicy {
            max_encoded_bytes: 2 * 1024 * 1024,
            max_dimension: 2048,
            max_pixels: 4_194_304,
            decoder_allocation: DecoderAllocation::IncludingOutput(32 * 1024 * 1024),
            formats: Formats::EnabledRasterCodecs,
            file_access: FileAccess::Follow,
            apply_orientation: true,
            thumbnail: Some(128),
        },
        RasterBudget {
            file_attempts: 2,
            encoded_bytes: 4 * 1024 * 1024,
            pixel_bytes: 32 * 1024 * 1024,
        },
        &|| false,
    );
    for path in paths {
        if let Some(raster) = batch.load(path)
            && let Some(image) =
                image::RgbaImage::from_raw(raster.width, raster.height, raster.pixels)
        {
            let side = image.width().min(image.height());
            let square = image::imageops::crop_imm(
                &image,
                (image.width() - side) / 2,
                (image.height() - side) / 2,
                side,
                side,
            );
            let image = image::imageops::resize(
                &square.to_image(),
                128,
                128,
                image::imageops::FilterType::Triangle,
            );
            return Some(image.into_raw());
        }
    }
    None
}
struct Shared {
    pending: Mutex<Option<(u64, u32)>>,
    wake: Condvar,
    epoch: AtomicU64,
    stopped: AtomicBool,
}
pub struct Worker {
    shared: Arc<Shared>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    pub fn start(notify: impl Fn(u64, Option<Identity>) + Send + 'static) -> Self {
        let shared = Arc::new(Shared {
            pending: Mutex::new(None),
            wake: Condvar::new(),
            epoch: AtomicU64::new(0),
            stopped: AtomicBool::new(false),
        });
        let state = shared.clone();
        let thread = std::thread::spawn(move || {
            loop {
                let (epoch, uid) = {
                    let mut pending = state.pending.lock().unwrap();
                    while pending.is_none() && !state.stopped.load(Ordering::Acquire) {
                        pending = state.wake.wait(pending).unwrap();
                    }
                    if state.stopped.load(Ordering::Acquire) {
                        break;
                    }
                    pending.take().unwrap()
                };
                let cancelled = || {
                    state.stopped.load(Ordering::Acquire)
                        || state.epoch.load(Ordering::Acquire) != epoch
                };
                let identity = (|| {
                    let mut command =
                        std::process::Command::new(canopa_file::executable::current().ok()?);
                    command.arg("internal-identity").arg(uid.to_string());
                    let bytes =
                        canopa_session_ui::helper::run(command, &cancelled, 300_000).ok()?;
                    let identity: Identity = serde_json::from_slice(&bytes).ok()?;
                    (identity.uid == uid
                        && valid_user(&identity.username)
                        && (identity.name.is_empty() || valid_name(&identity.name))
                        && identity
                            .rgba
                            .as_ref()
                            .is_none_or(|b| b.len() == 128 * 128 * 4))
                    .then_some(identity)
                })();
                if !cancelled() {
                    notify(epoch, identity);
                }
            }
        });
        Self {
            shared,
            thread: Some(thread),
        }
    }
    pub fn request(&self, uid: Option<u32>) -> u64 {
        let mut pending = self.shared.pending.lock().unwrap();
        let epoch = self.shared.epoch.fetch_add(1, Ordering::AcqRel) + 1;
        *pending = uid.map(|uid| (epoch, uid));
        self.shared.wake.notify_one();
        epoch
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let pending = self.shared.pending.lock().unwrap();
        self.shared.stopped.store(true, Ordering::Release);
        self.shared.wake.notify_one();
        drop(pending);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
