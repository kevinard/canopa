use canopa_assets::{
    DecoderAllocation, FileAccess, Formats, RasterBatch, RasterBudget, RasterPolicy,
};

#[test]
fn output_allocation_is_reserved_only_when_the_caller_requires_it() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("small.gif");
    image::RgbaImage::from_pixel(2, 2, image::Rgba([11, 22, 33, 255]))
        .save_with_format(&path, image::ImageFormat::Gif)
        .unwrap();
    let encoded_bytes = std::fs::metadata(&path).unwrap().len() as usize;

    let load = |decoder_allocation| {
        RasterBatch::new(
            RasterPolicy {
                max_encoded_bytes: encoded_bytes,
                max_dimension: 2,
                max_pixels: 4,
                decoder_allocation,
                formats: Formats::EnabledRasterCodecs,
                file_access: FileAccess::ResolveFirst,
                apply_orientation: false,
                thumbnail: None,
            },
            RasterBudget {
                file_attempts: 1,
                encoded_bytes,
                pixel_bytes: 16,
            },
            &|| false,
        )
        .load(&path)
    };

    // Full-canvas GIF decoding writes directly into the caller's RGBA buffer.
    // The decoder's own limits therefore do not reserve these 16 output bytes.
    let legacy = load(DecoderAllocation::DecoderOnly(15)).unwrap();
    assert_eq!(
        (legacy.width, legacy.height, legacy.pixels.len()),
        (2, 2, 16)
    );
    assert!(
        load(DecoderAllocation::IncludingOutput(15)).is_none(),
        "Center preparation must retain ImageReader::decode's output reservation"
    );
    assert_eq!(load(DecoderAllocation::IncludingOutput(16)), Some(legacy));
}
