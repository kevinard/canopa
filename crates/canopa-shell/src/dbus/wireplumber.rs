use crate::modules::volume::VolumeEvent;

struct VolumeConnData;

impl std::hash::Hash for VolumeConnData {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        "volume-subscription".hash(h);
    }
}

fn build_volume_stream(
    _data: &VolumeConnData,
) -> std::pin::Pin<Box<dyn iced::futures::Stream<Item = crate::app::Message> + Send + 'static>> {
    crate::dbus::subscription::build::<Option<(f64, bool)>, crate::app::Message, _, _, _>(
        |tx| {
            std::thread::spawn(move || canopa_services::audio::run_pa_thread(tx));
        },
        |reading| {
            let Some((volume, muted)) = reading else {
                return crate::app::Message::Modules(
                    crate::app::messages::ModuleRuntimeEvent::Volume(VolumeEvent::Unavailable),
                );
            };
            crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Volume(
                VolumeEvent::StateChanged { volume, muted },
            ))
        },
        || {
            crate::app::Message::Modules(crate::app::messages::ModuleRuntimeEvent::Volume(
                VolumeEvent::Unavailable,
            ))
        },
    )
}

/// Returns an Iced subscription for volume state via PulseAudio compat layer.
/// Emits Unavailable if PA socket is not reachable. No zbus connection needed.
pub(crate) fn volume_subscription() -> iced::Subscription<crate::app::Message> {
    iced::Subscription::run_with(VolumeConnData, build_volume_stream)
}

pub(crate) use canopa_services::audio::adjust_default_sink_volume;
