//! Immutable pixels and selection coordinates. No helper may choose a pathname.
use crate::{
    capture::{Captured, LogicalRect, MAX_BYTES, MAX_OUTPUTS, MAX_PIXELS, MAX_SIDE},
    screenshot::Kind,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::Write,
    os::{fd::AsRawFd, unix::fs::FileExt},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, watch};

mod cache;
#[cfg(test)]
mod tests;
pub(crate) use cache::Cache;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Frame {
    pub name: String,
    pub logical: LogicalRect,
    pub width: u32,
    pub height: u32,
    pub offset: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Selection {
    Image(Option<Rect>),
    ChooseWindow,
    Pixel { frame: usize, x: u32, y: u32 },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Source {
    pub descriptor: i32,
    pub bytes: u64,
    pub frames: Vec<Frame>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogInput {
    pub app_id: String,
    pub parent_window: String,
    pub modal: bool,
    pub kind: Kind,
    pub allow_area: bool,
    #[serde(default)]
    pub allow_window: bool,
    pub source: Source,
}

pub(crate) struct Snapshot {
    file: File,
    source: Source,
    _permit: OwnedSemaphorePermit,
}
impl Snapshot {
    pub(crate) async fn prepare(
        captured: Captured,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<Self> {
        task(cancelled, move |control| {
            let (frames, permit) = captured.into_parts();
            let original = rustix::fs::memfd_create(
                c"canopa-screenshot",
                rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
            )?;
            let mut file = File::from(rustix::io::fcntl_dupfd_cloexec(&original, 3)?);
            drop(original);
            let mut metadata = Vec::new();
            let mut offset = 0;
            for frame in frames {
                let (width, height) = frame.pixels.dimensions();
                metadata.push(Frame {
                    name: frame.name,
                    logical: frame.logical,
                    width,
                    height,
                    offset,
                });
                for chunk in frame.pixels.as_raw().chunks(65_536) {
                    control.check()?;
                    file.write_all(chunk)?;
                }
                offset += frame.pixels.as_raw().len() as u64;
            }
            let source = Source {
                descriptor: file.as_raw_fd(),
                bytes: offset,
                frames: metadata,
            };
            source.validate()?;
            rustix::fs::fcntl_add_seals(
                &file,
                rustix::fs::SealFlags::WRITE
                    | rustix::fs::SealFlags::GROW
                    | rustix::fs::SealFlags::SHRINK
                    | rustix::fs::SealFlags::SEAL,
            )?;
            Ok(Self {
                file,
                source,
                _permit: permit,
            })
        })
        .await
    }
    pub(crate) fn source(&self) -> Source {
        self.source.clone()
    }
    pub(crate) fn file(&self) -> &File {
        &self.file
    }
    pub(crate) fn color(&self, frame: usize, x: u32, y: u32) -> anyhow::Result<[f64; 3]> {
        let pixel = self.source.pixel(&self.file, frame, x, y)?;
        Ok([pixel[0], pixel[1], pixel[2]].map(|v| f64::from(v) / 255.0))
    }
    pub(crate) async fn publish(
        self,
        cache: Cache,
        rect: Option<Rect>,
        cancelled: watch::Receiver<bool>,
    ) -> anyhow::Result<String> {
        task(cancelled, move |control| {
            self.publish_sync(cache, rect, &control)
        })
        .await
    }
    fn publish_sync(
        self,
        cache: Cache,
        rect: Option<Rect>,
        control: &Control,
    ) -> anyhow::Result<String> {
        // Capture the whole snapshot, including admission, in detached work.
        let canvas = Canvas::new(&self.source.frames)?;
        let rect = rect.unwrap_or(canvas.full());
        let pixels = self.source.compose(&self.file, &canvas, rect, control)?;
        cache.publish(&pixels, rect.width, rect.height, control)
    }
}

impl Source {
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.frames.is_empty()
                && self.frames.len() <= MAX_OUTPUTS
                && self.bytes <= MAX_BYTES as u64,
            "Invalid screenshot source"
        );
        let mut offset = 0_u64;
        for frame in &self.frames {
            anyhow::ensure!(
                frame.width > 0
                    && frame.height > 0
                    && frame.width <= MAX_SIDE
                    && frame.height <= MAX_SIDE
                    && frame.logical.width > 0
                    && frame.logical.height > 0
                    && frame.logical.width <= MAX_SIDE
                    && frame.logical.height <= MAX_SIDE
                    && frame.logical.x.unsigned_abs() <= 131_072
                    && frame.logical.y.unsigned_abs() <= 131_072
                    && frame.offset == offset
                    && frame.name.len() <= 256
                    && !frame.name.chars().any(char::is_control),
                "Invalid screenshot frame"
            );
            offset = offset
                .checked_add(frame.width as u64 * frame.height as u64 * 4)
                .ok_or_else(|| anyhow::anyhow!("Screenshot exceeds limits"))?;
            anyhow::ensure!(offset <= MAX_BYTES as u64, "Screenshot exceeds limits");
        }
        anyhow::ensure!(offset == self.bytes, "Invalid screenshot byte count");
        Ok(())
    }
    pub(crate) fn open(&self) -> anyhow::Result<File> {
        self.validate()?;
        anyhow::ensure!(self.descriptor >= 3, "Invalid screenshot descriptor");
        // The descriptor is supplied by our backend, not a caller pathname.
        // Opening procfs safely rejects invalid numbers without borrowing an
        // unvalidated raw FD or taking ownership of another helper resource.
        let file = File::open(format!("/proc/self/fd/{}", self.descriptor))?;
        let metadata = file.metadata()?;
        let seals = rustix::fs::fcntl_get_seals(&file)?;
        anyhow::ensure!(
            metadata.is_file()
                && metadata.len() == self.bytes
                && seals.contains(
                    rustix::fs::SealFlags::WRITE
                        | rustix::fs::SealFlags::GROW
                        | rustix::fs::SealFlags::SHRINK
                        | rustix::fs::SealFlags::SEAL
                ),
            "Screenshot source is not immutable"
        );
        Ok(file)
    }
    pub(crate) fn pixel(
        &self,
        file: &File,
        index: usize,
        x: u32,
        y: u32,
    ) -> anyhow::Result<[u8; 4]> {
        let frame = self
            .frames
            .get(index)
            .ok_or_else(|| anyhow::anyhow!("Invalid screenshot pixel"))?;
        anyhow::ensure!(
            x < frame.width && y < frame.height,
            "Invalid screenshot pixel"
        );
        let mut pixel = [0; 4];
        file.read_exact_at(
            &mut pixel,
            frame.offset + (y as u64 * frame.width as u64 + x as u64) * 4,
        )?;
        Ok(pixel)
    }
    pub(crate) fn previews(&self, file: &File) -> anyhow::Result<Vec<image::RgbaImage>> {
        let side = ((4_194_304_f64 / self.frames.len() as f64).sqrt() as u32).min(1024);
        self.frames
            .iter()
            .map(|frame| {
                let ratio = (side as f64 / frame.width as f64)
                    .min(side as f64 / frame.height as f64)
                    .min(1.0);
                let w = (frame.width as f64 * ratio).floor().max(1.0) as u32;
                let h = (frame.height as f64 * ratio).floor().max(1.0) as u32;
                let mut pixels = vec![0; w as usize * h as usize * 4];
                let mut row = vec![0; frame.width as usize * 4];
                for y in 0..h {
                    let source_y = (y as u64 * frame.height as u64 / h as u64) as u32;
                    file.read_exact_at(
                        &mut row,
                        frame.offset + source_y as u64 * frame.width as u64 * 4,
                    )?;
                    for x in 0..w {
                        let source_x = (x as u64 * frame.width as u64 / w as u64) as usize;
                        let dst = (y as usize * w as usize + x as usize) * 4;
                        pixels[dst..dst + 4].copy_from_slice(&row[source_x * 4..source_x * 4 + 4]);
                    }
                }
                image::RgbaImage::from_raw(w, h, pixels)
                    .ok_or_else(|| anyhow::anyhow!("Invalid screenshot preview"))
            })
            .collect()
    }
    fn compose(
        &self,
        file: &File,
        canvas: &Canvas,
        rect: Rect,
        control: &Control,
    ) -> anyhow::Result<Vec<u8>> {
        rect.validate(canvas)?;
        let count = rect.width as usize * rect.height as usize * 4;
        let mut pixels = Vec::new();
        pixels.try_reserve_exact(count)?;
        pixels.resize(count, 0);
        let mut touched = false;
        for frame in &self.frames {
            let bounds = canvas.bounds(frame);
            let left = bounds.x.floor().max(rect.x as f64) as u32;
            let top = bounds.y.floor().max(rect.y as f64) as u32;
            let right = (bounds.x + bounds.width)
                .ceil()
                .min((rect.x + rect.width) as f64) as u32;
            let bottom = (bounds.y + bounds.height)
                .ceil()
                .min((rect.y + rect.height) as f64) as u32;
            let mut row = vec![0; frame.width as usize * 4];
            for y in top..bottom {
                control.check()?;
                let cy = y as f64 + 0.5;
                if cy < bounds.y || cy >= bounds.y + bounds.height {
                    continue;
                }
                let source_y = (((cy - bounds.y) / bounds.height * frame.height as f64).floor()
                    as u32)
                    .min(frame.height - 1);
                file.read_exact_at(
                    &mut row,
                    frame.offset + source_y as u64 * frame.width as u64 * 4,
                )?;
                for x in left..right {
                    let cx = x as f64 + 0.5;
                    if cx < bounds.x || cx >= bounds.x + bounds.width {
                        continue;
                    }
                    let source_x = (((cx - bounds.x) / bounds.width * frame.width as f64).floor()
                        as usize)
                        .min(frame.width as usize - 1);
                    let target =
                        ((y - rect.y) as usize * rect.width as usize + (x - rect.x) as usize) * 4;
                    pixels[target..target + 4]
                        .copy_from_slice(&row[source_x * 4..source_x * 4 + 4]);
                    touched = true;
                }
            }
        }
        anyhow::ensure!(touched, "Screenshot area does not include a display");
        Ok(pixels)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Debug)]
pub(crate) struct Canvas {
    pub width: u32,
    pub height: u32,
    x: i32,
    y: i32,
    sx: f64,
    sy: f64,
}
impl Canvas {
    pub(crate) fn has_pixels(&self, frames: &[Frame], rect: Rect) -> bool {
        frames.iter().any(|frame| {
            let b = self.bounds(frame);
            let x = (b.x.max(rect.x as f64) - 0.5).ceil() + 0.5;
            let y = (b.y.max(rect.y as f64) - 0.5).ceil() + 0.5;
            x < (b.x + b.width).min((rect.x + rect.width) as f64)
                && y < (b.y + b.height).min((rect.y + rect.height) as f64)
        })
    }
    pub(crate) fn new(frames: &[Frame]) -> anyhow::Result<Self> {
        anyhow::ensure!(!frames.is_empty(), "Screenshot has no display");
        let x = frames
            .iter()
            .map(|f| f.logical.x)
            .min()
            .expect("nonempty frames");
        let y = frames
            .iter()
            .map(|f| f.logical.y)
            .min()
            .expect("nonempty frames");
        let right = frames
            .iter()
            .map(|f| f.logical.x as i64 + f.logical.width as i64)
            .max()
            .expect("nonempty frames");
        let bottom = frames
            .iter()
            .map(|f| f.logical.y as i64 + f.logical.height as i64)
            .max()
            .expect("nonempty frames");
        let scale = frames
            .iter()
            .map(|f| {
                (f.width as f64 / f.logical.width as f64)
                    .max(f.height as f64 / f.logical.height as f64)
            })
            .fold(0.0_f64, f64::max);
        let (sx, sy) = if frames.len() == 1 {
            (
                frames[0].width as f64 / frames[0].logical.width as f64,
                frames[0].height as f64 / frames[0].logical.height as f64,
            )
        } else {
            (scale, scale)
        };
        let width = (right - x as i64) as f64 * sx;
        let height = (bottom - y as i64) as f64 * sy;
        anyhow::ensure!(
            width.is_finite()
                && height.is_finite()
                && width > 0.0
                && height > 0.0
                && width.ceil() <= u32::MAX as f64
                && height.ceil() <= u32::MAX as f64,
            "Screenshot layout exceeds limits"
        );
        // A single output retains every native row/column despite rounded logical size.
        let (width, height) = if frames.len() == 1 {
            (frames[0].width, frames[0].height)
        } else {
            (width.ceil() as u32, height.ceil() as u32)
        };
        Ok(Self {
            width,
            height,
            x,
            y,
            sx,
            sy,
        })
    }
    pub(crate) fn full(&self) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
        }
    }
    pub(crate) fn bounds(&self, frame: &Frame) -> Bounds {
        Bounds {
            x: (frame.logical.x as f64 - self.x as f64) * self.sx,
            y: (frame.logical.y as f64 - self.y as f64) * self.sy,
            width: frame.logical.width as f64 * self.sx,
            height: frame.logical.height as f64 * self.sy,
        }
    }
    pub(crate) fn pixel_at(&self, frames: &[Frame], x: f64, y: f64) -> Option<(usize, u32, u32)> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        frames.iter().enumerate().rev().find_map(|(index, frame)| {
            let b = self.bounds(frame);
            (x >= b.x && y >= b.y && x < b.x + b.width && y < b.y + b.height).then(|| {
                (
                    index,
                    ((x - b.x) / b.width * frame.width as f64).floor() as u32,
                    ((y - b.y) / b.height * frame.height as f64).floor() as u32,
                )
            })
        })
    }
}
impl Rect {
    pub(crate) fn validate(&self, canvas: &Canvas) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.width > 0
                && self.height > 0
                && self.width as u64 * self.height as u64 <= MAX_PIXELS as u64
                && self
                    .x
                    .checked_add(self.width)
                    .is_some_and(|right| right <= canvas.width)
                && self
                    .y
                    .checked_add(self.height)
                    .is_some_and(|bottom| bottom <= canvas.height),
            "Screenshot area exceeds limits"
        );
        Ok(())
    }
}

pub(crate) struct Control {
    stop: Arc<AtomicBool>,
    end: Instant,
}
impl Control {
    fn check(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.stop.load(Ordering::Relaxed) && Instant::now() < self.end,
            "Screenshot work cancelled or timed out"
        );
        Ok(())
    }
}
struct Stop(Arc<AtomicBool>);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
async fn task<T: Send + 'static>(
    mut cancelled: watch::Receiver<bool>,
    work: impl FnOnce(Control) -> anyhow::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    anyhow::ensure!(
        !*cancelled.borrow() && cancelled.has_changed().is_ok(),
        "Screenshot cancelled"
    );
    let stop = Stop(Arc::new(AtomicBool::new(false)));
    let control = Control {
        stop: stop.0.clone(),
        end: Instant::now() + Duration::from_secs(30),
    };
    let job = tokio::task::spawn_blocking(move || work(control));
    tokio::select! { biased;
        _ = cancelled.changed() => anyhow::bail!("Screenshot cancelled"),
        _ = tokio::time::sleep(Duration::from_secs(30)) => anyhow::bail!("Screenshot work timed out"),
        result = job => result?,
    }
}
