# Canopa System services. English is the source language.

## Errors shown by the Shell and Settings when a system service fails.
## Lower-case ones may follow a short context such as "Bluetooth: ".

services-error-unavailable = the service is not available
services-error-not-authorized = the request is not authorized
services-error-timed-out = the service did not answer in time
# The device, profile or player the request named is no longer current.
services-error-changed = the target changed

## Audio (PulseAudio/PipeWire), shown in the Shell's sound panel.

services-audio-busy = Audio controls are busy or unavailable.
services-audio-timed-out = Audio operation timed out.
services-audio-controls-unavailable = Audio controls are unavailable.
services-audio-unavailable = Audio is unavailable.
services-audio-no-response = Audio server did not respond.
services-audio-too-many-devices = Too many audio devices.
services-audio-read-failed = Could not read audio devices.
services-audio-connection-changed = The audio connection changed. Choose a current device.
services-audio-device-gone = The audio device is no longer available. Choose a current device.
services-audio-invalid-channels = Invalid audio channel volumes.
services-audio-volume-out-of-range = Volume must be between 0% and 100%.
services-audio-rejected = The audio server rejected the change.
services-audio-no-default-sink = default audio sink is unavailable
services-audio-adjustment-rejected = audio server rejected volume adjustment
services-audio-adjustment-timed-out = audio adjustment timed out
# Name of an audio stream whose application gave no name.
services-audio-unknown-application = Unknown application
# Name of Canopa's microphone level stream, as mixers list it.
services-audio-meter-stream = Input level

## Media players (MPRIS), shown in the Shell's media controls.

services-mpris-changed = Player or track changed; try again
services-mpris-unsupported = This player does not support that control
# $detail is a short English technical description.
services-mpris-invalid = The player sent invalid information ({ $detail })

## Session actions (logind).

services-logind-unavailable = the login manager is not available
# Applications hold inhibitors that block the action.
services-logind-blocked = blocked by applications
services-logind-timed-out = the login manager did not answer in time
# The login manager refused without saying why.
services-logind-refused = The login manager refused the request.
# Reason given to logind (and shown by tools such as systemd-inhibit) while
# Canopa handles the lid switch itself.
services-lid-inhibitor-reason = Closing the lid follows the choice in Canopa Settings

## Devices and secrets

# Name of a Bluetooth device that reports neither a name nor an address.
services-bluetooth-unnamed-device = Unnamed device
# Labels of the fields NetworkManager asks for when connecting.
services-secret-wep-key = WEP key
services-secret-wifi-password = Wi-Fi password
services-secret-private-key-password = Private key password
services-secret-password = Password
services-secret-wireguard-key = WireGuard private key
services-secret-vpn-password = VPN password
