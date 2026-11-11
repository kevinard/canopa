//! Iced handles for icons prepared by `canopa-assets`. Lookup, reads and decoding
//! stay in the UI-free crates and run on the caller's worker; views only clone
//! the handles made here.
use canopa_assets::Raster;
use iced::widget::image::Handle;

/// An Iced image handle owning the prepared straight-RGBA pixels.
pub fn handle(raster: Raster) -> Handle {
    Handle::from_rgba(raster.width, raster.height, raster.pixels)
}

/// Like [`handle`], first removing transparent margins so icons with
/// different padding align at the same size.
pub fn trimmed_handle(raster: Raster) -> Handle {
    handle(raster.trimmed())
}

#[cfg(test)]
mod tests {
    #[test]
    fn handles_keep_prepared_pixels_and_trim_padding() {
        let raster = canopa_assets::Raster {
            width: 2,
            height: 1,
            pixels: vec![0, 0, 0, 0, 1, 2, 3, 255],
        };
        let Handle::Rgba { width, pixels, .. } = super::trimmed_handle(raster.clone()) else {
            panic!("expected prepared pixels");
        };
        assert_eq!((width, pixels.as_ref()), (1, &[1, 2, 3, 255][..]));
        let Handle::Rgba { width, .. } = super::handle(raster) else {
            panic!("expected prepared pixels");
        };
        assert_eq!(width, 2);
    }
    use iced::widget::image::Handle;
}
