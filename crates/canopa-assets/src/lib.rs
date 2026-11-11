//! Bounded local raster preparation, independent of GUI and application lifetimes.
//!
//! Callers own worker admission, cancellation, fallback presentation and the lifetime
//! of each batch. A batch does synchronous filesystem and decoder work: run it on
//! an admitted blocking worker. Limits bound logical work and output, not filesystem
//! latency or total process memory. No paths or image content are logged.

use image::{ImageDecoder, ImageFormat, ImageReader};
use std::io::Cursor;
use std::path::Path;

pub mod icons;
#[cfg(feature = "svg")]
pub mod svg;

/// File formats admitted independently of Cargo feature unification.
#[derive(Clone, Copy)]
pub enum Formats {
    /// Require a PNG/JPEG filename and PNG/JPEG content, including through symlinks.
    StaticPngJpeg,
    /// Require PNG/JPEG content, whatever the filename (extensionless temp files).
    SniffedPngJpeg,
    /// Accept the raster codecs enabled in the consumer's compiled image graph.
    EnabledRasterCodecs,
}

/// How local symlinks are handled before the regular-file descriptor is read.
#[derive(Clone, Copy)]
pub enum FileAccess {
    /// Follow symlinks while opening, then validate the opened descriptor.
    Follow,
    /// Resolve symlinks first and reject a replaced final symlink while opening.
    ResolveFirst,
}

/// Decoder limits differ in whether the destination buffer is charged up front.
#[derive(Clone, Copy)]
pub enum DecoderAllocation {
    /// The codec receives the full limit; source RGBA admission remains separate.
    DecoderOnly(u64),
    /// Reserve the native decoded buffer, then give the codec the remainder.
    IncludingOutput(u64),
}

impl DecoderAllocation {
    fn maximum(self) -> u64 {
        match self {
            Self::DecoderOnly(bytes) | Self::IncludingOutput(bytes) => bytes,
        }
    }
}

/// Per-image limits and presentation choices, supplied by the consumer.
#[derive(Clone, Copy)]
pub struct RasterPolicy {
    pub max_encoded_bytes: usize,
    pub max_dimension: u32,
    pub max_pixels: u64,
    /// The image crate's best-effort allocation limit and output accounting mode.
    pub decoder_allocation: DecoderAllocation,
    pub formats: Formats,
    pub file_access: FileAccess,
    pub apply_orientation: bool,
    /// Fit oversized output inside this square, preserving aspect ratio.
    pub thumbnail: Option<u32>,
}

/// Aggregate limits for a single caller-owned batch, across files and raw pixels.
pub struct RasterBudget {
    pub file_attempts: usize,
    pub encoded_bytes: usize,
    /// Source RGBA size is charged before thumbnailing; failed decoding costs no pixels.
    pub pixel_bytes: usize,
}

/// Owned pixels remain unchanged when the source file is edited or removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Raster {
    /// Paint every pixel `color`, keeping its coverage: symbolic icon recoloring.
    pub fn recolor(&mut self, color: [u8; 4]) {
        for pixel in self.pixels.chunks_exact_mut(4) {
            pixel[..3].copy_from_slice(&color[..3]);
            pixel[3] = ((u16::from(pixel[3]) * u16::from(color[3])) / 255) as u8;
        }
    }

    /// Remove fully transparent margins; an empty or unpadded raster is unchanged.
    pub fn trimmed(self) -> Self {
        let (width, height) = (self.width, self.height);
        let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
        for (index, pixel) in self.pixels.chunks_exact(4).enumerate() {
            if pixel[3] == 0 {
                continue;
            }
            let x = index as u32 % width;
            let y = index as u32 / width;
            left = left.min(x);
            top = top.min(y);
            right = right.max(x + 1);
            bottom = bottom.max(y + 1);
        }
        if right <= left || bottom <= top || (left, top, right, bottom) == (0, 0, width, height) {
            return self;
        }
        let mut pixels = Vec::with_capacity(((right - left) * (bottom - top) * 4) as usize);
        for y in top..bottom {
            let start = ((y * width + left) * 4) as usize;
            let end = ((y * width + right) * 4) as usize;
            pixels.extend_from_slice(&self.pixels[start..end]);
        }
        Self {
            width: right - left,
            height: bottom - top,
            pixels,
        }
    }
}

/// A bounded preparation batch with no cache, queue, executor or background tasks.
pub struct RasterBatch<'a> {
    policy: RasterPolicy,
    remaining: RasterBudget,
    cancelled: &'a dyn Fn() -> bool,
}

impl<'a> RasterBatch<'a> {
    pub fn new(
        policy: RasterPolicy,
        budget: RasterBudget,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Self {
        Self {
            policy,
            remaining: budget,
            cancelled,
        }
    }

    /// Read and decode a local file. Failure consumes the attempt and any bytes read,
    /// including an overflow probe. Missing, invalid and over-budget files return None.
    /// Cancellation is cooperative between stages; an active codec is not preempted.
    pub fn load(&mut self, path: &Path) -> Option<Raster> {
        let bytes = self.read(path)?;
        self.decode_inner(bytes)
    }

    /// Like [`Self::load`], but SVG content (detected from the bytes, never the
    /// name) is rasterized with `svg` and charged to the same pixel budget.
    #[cfg(feature = "svg")]
    pub fn load_image_or_svg(&mut self, path: &Path, svg: &svg::Policy) -> Option<Raster> {
        let bytes = self.read(path)?;
        if !svg::is_svg(&bytes) {
            return self.decode_inner(bytes);
        }
        let raster = svg::rasterize(&bytes, svg.clone())?;
        self.admit_rgba(raster.width, raster.height, raster.pixels.len())
            .then_some(raster)
    }

    fn read(&mut self, path: &Path) -> Option<Vec<u8>> {
        if (self.cancelled)() {
            return None;
        }
        self.remaining.file_attempts = self.remaining.file_attempts.checked_sub(1)?;
        if !path.is_absolute() || self.remaining.encoded_bytes == 0 {
            return None;
        }
        if matches!(self.policy.formats, Formats::StaticPngJpeg) {
            let extension = path.extension()?.to_str()?;
            if !["png", "jpg", "jpeg"]
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
            {
                return None;
            }
        }
        let maximum = self
            .policy
            .max_encoded_bytes
            .min(self.remaining.encoded_bytes);
        let read = match self.policy.file_access {
            FileAccess::Follow => canopa_file::read_bounded(path, maximum),
            FileAccess::ResolveFirst => canopa_file::read_canonical_bounded(path, maximum),
        };
        let consumed = read
            .as_ref()
            .map(Vec::len)
            .unwrap_or_else(|error| error.bytes_read);
        self.remaining.encoded_bytes = self.remaining.encoded_bytes.saturating_sub(consumed);
        let bytes = read.ok()?;
        if (self.cancelled)() {
            return None;
        }
        Some(bytes)
    }

    /// Decode an already-owned payload with the same format, allocation and pixel
    /// bounds as local files. The attempt and encoded bytes consume this batch's
    /// budget even when decoding fails.
    pub fn decode(&mut self, bytes: Vec<u8>) -> Option<Raster> {
        if (self.cancelled)() {
            return None;
        }
        self.remaining.file_attempts = self.remaining.file_attempts.checked_sub(1)?;
        if bytes.len() > self.policy.max_encoded_bytes || bytes.len() > self.remaining.encoded_bytes
        {
            return None;
        }
        self.remaining.encoded_bytes -= bytes.len();
        self.decode_inner(bytes)
    }

    fn decode_inner(&mut self, bytes: Vec<u8>) -> Option<Raster> {
        let mut reader = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        if matches!(
            self.policy.formats,
            Formats::StaticPngJpeg | Formats::SniffedPngJpeg
        ) && !matches!(reader.format(), Some(ImageFormat::Png | ImageFormat::Jpeg))
        {
            return None;
        }
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(self.policy.max_dimension);
        limits.max_image_height = Some(self.policy.max_dimension);
        limits.max_alloc = Some(self.policy.decoder_allocation.maximum());
        reader.limits(limits.clone());
        let mut decoder = reader.into_decoder().ok()?;
        let (width, height) = decoder.dimensions();
        let pixel_bytes = self.pixel_bytes(width, height)?;
        if pixel_bytes > self.remaining.pixel_bytes || (self.cancelled)() {
            return None;
        }
        if matches!(
            self.policy.decoder_allocation,
            DecoderAllocation::IncludingOutput(_)
        ) {
            // Match ImageReader::decode: native output may be larger than RGBA8
            // (e.g. floating-point images), and codecs can need scratch buffers.
            limits.reserve(decoder.total_bytes()).ok()?;
            decoder.set_limits(limits).ok()?;
        }
        let orientation = if self.policy.apply_orientation {
            decoder
                .exif_metadata()
                .ok()
                .flatten()
                .as_deref()
                .and_then(image::metadata::Orientation::from_exif_chunk)
                .unwrap_or(image::metadata::Orientation::NoTransforms)
        } else {
            image::metadata::Orientation::NoTransforms
        };
        let mut decoded = image::DynamicImage::from_decoder(decoder).ok()?;
        if (self.cancelled)() {
            return None;
        }
        decoded.apply_orientation(orientation);
        if let Some(side) = self.policy.thumbnail {
            if side == 0 {
                return None;
            }
            if decoded.width() > side || decoded.height() > side {
                decoded = decoded.thumbnail(side, side);
            }
        }
        let pixels = decoded.into_rgba8();
        if (self.cancelled)() {
            return None;
        }
        self.remaining.pixel_bytes -= pixel_bytes;
        Some(Raster {
            width: pixels.width(),
            height: pixels.height(),
            pixels: pixels.into_raw(),
        })
    }

    /// Admit already-owned RGBA bytes into the same pixel budget as decoded files.
    pub fn admit_rgba(&mut self, width: u32, height: u32, byte_count: usize) -> bool {
        if (self.cancelled)() {
            return false;
        }
        let Some(bytes) = self.pixel_bytes(width, height) else {
            return false;
        };
        if bytes != byte_count || bytes > self.remaining.pixel_bytes {
            return false;
        }
        self.remaining.pixel_bytes -= bytes;
        true
    }

    fn pixel_bytes(&self, width: u32, height: u32) -> Option<usize> {
        let pixels = u64::from(width) * u64::from(height);
        if width > self.policy.max_dimension
            || height > self.policy.max_dimension
            || pixels == 0
            || pixels > self.policy.max_pixels
        {
            return None;
        }
        usize::try_from(pixels.checked_mul(4)?).ok()
    }
}
