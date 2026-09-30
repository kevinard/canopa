# Sound controls

Click the volume module, or activate it from the keyboard, to open **Sound**.
The panel lists speakers/headphones and microphones with their volume, mute state
and current default marker. Output monitor sources are excluded from microphone
choices.

- **Volume −5% / +5%** changes the loudest channel in five percentage-point steps,
  keeping channel proportions. The displayed percentage is the channel average,
  matching the bar. Changes stop at 0–100% and do not automatically unmute.
- **Mute / Unmute** explicitly changes that device's mute state.
- **Use this device** requests the default output or input. Existing application
  streams follow the audio session manager's routing policy; Canopa does not move
  them individually.

The panel follows external changes and device removal. It shows an unavailable
message while reconnecting after audio-server loss. Failed operations display an
error. Keyboard focus remains on an existing control after a live update; Tab and
the panel's existing keyboard navigation can reach each action, and Escape closes.

Both the bar and panel work with PulseAudio or PipeWire's PulseAudio compatibility
service. Install `canopa-audio-provider` beside `canopa-shell` or on PATH. No `pactl`
or `wpctl` executable is required for production controls. Microphone discovery
reads device information; it does not record audio.

Existing volume `actions { click { open-panel; } }` bindings open Sound. Explicit
custom commands or other actions still take precedence. To open Sound from another
module or a named menu, use `open-provider "audio"`; no provider declaration is
required. A configured provider named `audio` overrides the built-in.

This milestone covers devices. Application mixing, audio-card profiles, connector
selection and continuous volume sliders remain future work.

## Validation

The library's `audio` tests use private PipeWire/Pulse servers with synthetic
outputs and remapped inputs. They cover speaker and microphone volume/mute,
channel proportions, default requests, external changes, monitor exclusion,
removal, reconnection and stale actions through the v5 provider transport.
The fixture supplies active-default metadata because it has no session manager.
Physical devices and native PulseAudio hardware behavior require desktop acceptance.
