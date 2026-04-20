use super::proxy::SniPixmap;

pub(super) const MAX_PIXMAP_DIMENSION: i32 = 512;
pub(super) const MAX_RETAINED_PIXMAP_COUNT: usize = 8;
pub(super) const MAX_RETAINED_PIXMAP_BYTES: usize = 256 * 1024;

pub(super) fn valid_pixmap(width: i32, height: i32, bytes: &[u8]) -> bool {
    if width <= 0 || height <= 0 {
        return false;
    }
    if width > MAX_PIXMAP_DIMENSION || height > MAX_PIXMAP_DIMENSION {
        return false;
    }
    let Some(pixel_count) = (width as usize).checked_mul(height as usize) else {
        return false;
    };
    let Some(expected_bytes) = pixel_count.checked_mul(4) else {
        return false;
    };
    bytes.len() == expected_bytes
}

pub(super) fn retain_pixmaps_with_budget(
    pixmaps: Vec<SniPixmap>,
    max_count: usize,
    max_bytes: usize,
) -> Vec<SniPixmap> {
    let mut retained = Vec::new();
    let mut retained_bytes = 0usize;

    for (width, height, bytes) in pixmaps {
        if retained.len() >= max_count {
            break;
        }
        if !valid_pixmap(width, height, &bytes) {
            continue;
        }

        let pixmap_bytes = bytes.len();
        if pixmap_bytes > max_bytes || retained_bytes.saturating_add(pixmap_bytes) > max_bytes {
            continue;
        }

        retained_bytes += pixmap_bytes;
        retained.push((width, height, bytes));
    }

    retained
}

/// Convert ARGB big-endian bytes (SNI wire format) to RGBA bytes (iced format).
pub(crate) fn argb_to_rgba(argb: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(argb.len());
    for chunk in argb.chunks_exact(4) {
        // ARGB: chunk[0]=A, chunk[1]=R, chunk[2]=G, chunk[3]=B
        rgba.push(chunk[1]); // R
        rgba.push(chunk[2]); // G
        rgba.push(chunk[3]); // B
        rgba.push(chunk[0]); // A
    }
    rgba
}

/// Pick the pixmap whose width is closest to `target_size`.
/// Returns a reference to the chosen (width, height, bytes) tuple.
pub(crate) fn pick_best_pixmap(
    maps: &[(i32, i32, Vec<u8>)],
    target_size: i32,
) -> Option<&(i32, i32, Vec<u8>)> {
    maps.iter()
        .min_by_key(|(w, _, _)| (*w - target_size).unsigned_abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argb_to_rgba_converts_channels() {
        let argb = vec![0xAA, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77];
        assert_eq!(
            argb_to_rgba(&argb),
            vec![0x11, 0x22, 0x33, 0xAA, 0x55, 0x66, 0x77, 0x44]
        );
    }

    #[test]
    fn argb_to_rgba_handles_empty() {
        assert_eq!(argb_to_rgba(&[]), Vec::<u8>::new());
    }

    #[test]
    fn valid_pixmap_accepts_matching_byte_len() {
        assert!(valid_pixmap(16, 16, &[0u8; 16 * 16 * 4]));
    }

    #[test]
    fn valid_pixmap_rejects_negative_dimensions() {
        assert!(!valid_pixmap(-1, 16, &[0u8; 16 * 16 * 4]));
    }

    #[test]
    fn valid_pixmap_rejects_mismatched_byte_len() {
        assert!(!valid_pixmap(16, 16, &[0u8; 3]));
    }

    #[test]
    fn valid_pixmap_rejects_zero_dimensions() {
        assert!(!valid_pixmap(0, 16, &[]));
        assert!(!valid_pixmap(16, 0, &[]));
    }

    #[test]
    fn valid_pixmap_rejects_height_out_of_range() {
        let bytes = vec![0u8; (MAX_PIXMAP_DIMENSION as usize + 1) * 16 * 4];
        assert!(!valid_pixmap(16, MAX_PIXMAP_DIMENSION + 1, &bytes));
    }

    #[test]
    fn valid_pixmap_accepts_max_dimension() {
        let dim = MAX_PIXMAP_DIMENSION as usize;
        let bytes = vec![0u8; dim * dim * 4];
        assert!(valid_pixmap(
            MAX_PIXMAP_DIMENSION,
            MAX_PIXMAP_DIMENSION,
            &bytes
        ));
    }

    #[test]
    fn retain_pixmaps_with_budget_skips_invalid_and_limits_count() {
        let pixmaps = vec![
            (1, 1, vec![0u8; 3]),
            (16, 16, vec![0u8; 16 * 16 * 4]),
            (22, 22, vec![0u8; 22 * 22 * 4]),
            (32, 32, vec![0u8; 32 * 32 * 4]),
        ];
        let retained = retain_pixmaps_with_budget(pixmaps, 2, usize::MAX);
        assert_eq!(retained.len(), 2);
        assert_eq!(retained[0].0, 16);
        assert_eq!(retained[1].0, 22);
    }

    #[test]
    fn retain_pixmaps_with_budget_limits_total_bytes() {
        let pixmaps = vec![
            (16, 16, vec![0u8; 16 * 16 * 4]),
            (22, 22, vec![0u8; 22 * 22 * 4]),
            (32, 32, vec![0u8; 32 * 32 * 4]),
        ];
        let retained = retain_pixmaps_with_budget(pixmaps, 8, 16 * 16 * 4 + 22 * 22 * 4);
        assert_eq!(retained.len(), 2);
        assert_eq!(retained[0].0, 16);
        assert_eq!(retained[1].0, 22);
    }

    #[test]
    fn pick_best_pixmap_prefers_closest_to_target() {
        let maps = vec![
            (16i32, 16i32, vec![0u8; 16 * 16 * 4]),
            (22i32, 22i32, vec![0u8; 22 * 22 * 4]),
            (32i32, 32i32, vec![0u8; 32 * 32 * 4]),
        ];
        let best = pick_best_pixmap(&maps, 20);
        assert_eq!(best.map(|b| b.0), Some(22));
    }

    #[test]
    fn pick_best_pixmap_returns_none_for_empty() {
        let maps: Vec<(i32, i32, Vec<u8>)> = vec![];
        assert!(pick_best_pixmap(&maps, 20).is_none());
    }
}
