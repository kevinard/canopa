#[cfg(test)]
mod test_bus;
pub mod external_panel {
    #[cfg(feature = "desktop")]
    pub mod canopa;
    #[cfg(feature = "desktop")]
    mod canopa_images;
    pub mod protocol;
}

pub mod wifi_provider;

pub mod audio;
pub mod audio_provider;

pub mod media;
pub mod media_provider;

mod provider_io;

#[cfg(feature = "desktop")]
pub mod hardware_panel;
