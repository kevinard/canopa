#[cfg(test)]
mod test_bus;
pub mod external_panel {
    pub mod canopa;
    mod canopa_images;
    pub mod protocol;
}

pub mod wifi_provider;

pub mod audio;
pub mod audio_provider;

pub mod media;
pub mod media_provider;
