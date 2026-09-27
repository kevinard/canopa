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

/// File formats admitted independently of Cargo feature unification.
#[derive(Clone, Copy)]
pub enum Formats {
    /// Require a PNG/JPEG filename and PNG/JPEG content, including through symlinks.
    StaticPngJpeg,
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
#[derive(Debug, PartialEq, Eq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
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
        let mut reader = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        if matches!(self.policy.formats, Formats::StaticPngJpeg)
            && !matches!(reader.format(), Some(ImageFormat::Png | ImageFormat::Jpeg))
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
