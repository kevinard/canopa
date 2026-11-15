# Audio device controls use a provider and a live Pulse connection

The built-in `audio` provider owns speaker/headphone and microphone controls.
It follows ADR 0010 and sends existing `canopa-v5` rich content; no audio-specific
widgets, protocol variants or executable actions are added to the host. This first
device milestone uses accessible volume decrease/increase, mute and default-device
buttons. Application mixing, profiles, port selection and continuous sliders are
separate work.

The bar and provider share the `canopa-shell` library's libpulse connection code.
Both respect the same PulseAudio server selection, including PipeWire's compatibility
server. The bar retains its lightweight default-output observer and scroll controls.
The provider has a single connection-owning worker, a bounded command queue and a
latest-snapshot channel. It subscribes to output, input and default-device changes;
discovery opens no capture stream. Monitor sources are omitted from microphone choices.

Device controls carry the connection generation, server cookie, device kind, index
and name. The worker rejects a generation from an earlier connection and re-queries
the exact device before writing. Server cookies alone are insufficient: restarting
PipeWire's PulseAudio endpoint can retain the core cookie. No operation follows a
new default or silently substitutes another device. Commands are serialized, bounded,
acknowledged and errors are visible in the panel. Disconnect clears the published
state and triggers reconnection; closing the panel drops its observer.

Volume adjustments preserve channel proportions and cap the loudest channel at
100%. They leave mute unchanged. Choosing a default requests the audio server's
default for future routing; moving already-running application streams remains
the audio session manager's policy. Canopa does not migrate streams in this milestone.
PulseAudio documents [channel-preserving scaling](https://freedesktop.org/software/pulseaudio/doxygen/volume_8h.html).

Volume modules open Sound on primary activation. Existing module `open-panel`
bindings resolve to the audio provider; other explicit actions remain authoritative.
Configured `audio` providers override the built-in as usual. The provider is included
in release archives and the updater.

Live rich-content updates preserve keyboard focus when the same opaque action
target/key/value still exists in the same provider session. Generation-scoped action
IDs are always renewed, so preserving focus never makes an old event valid.

The slider follow-up replaces volume buttons with an absolute control using the
shared optional rich-item slider (Canopa ADR 0012; see [the panel protocol](../../architecture/panel-protocol.md)).
The initial button choice above records the first milestone, not the current UI.

The shared libpulse connection code now lives in `canopa_services::audio`
(shared ADR 0137); the behaviour above is unchanged.
