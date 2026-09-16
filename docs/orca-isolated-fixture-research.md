# Isolated Orca and speech fixture

Research date: 2026-09-16. This note records a runnable acceptance toolchain under `/tmp`; it does not install packages or certify the product's full accessibility behavior.

## Prepared toolchain

The existing Arch repository metadata returned package URLs with:

```sh
pacman -Sp orca speech-dispatcher espeak-ng --print-format '%n %v %l'
```

Downloaded packages were extracted under `/tmp/canopa-orca-fixture/root`, with each archive's SHA-256 verified against `/var/lib/pacman/sync/extra.db` before extraction. No package hooks ran. The exact URL/version/hash inventory is `/tmp/canopa-orca-fixture/package-manifest.json`; archives remain in its `packages/` directory. The source is the Arch-managed `https://london.mirror.pkgbuild.com/extra/os/x86_64/` mirror.

| Package | Verified version |
| --- | --- |
| orca | 50.2-1 |
| speech-dispatcher, libspeechd | 0.12.1-3 |
| espeak-ng | 1.52.0-1 |
| python-dasbus | 1.7-5 |
| python-setproctitle | 1.3.7-2 |
| dotconf | 1.4.1-1 |
| pcaudiolib | 1.3-1 |
| libsonic | 0.2.0-2 |
| libwnck3 | 43.3-1 |
| libxres | 1.2.3-1 |

The metadata's liblouis, brltty and python-brltty URLs returned 404. They were not installed or required for this speech test. Braille and Wnck are optional upstream dependencies. Existing system Python 3.14.5, PyGObject, GTK3, AT-SPI, Cairo, GLib and audio libraries supply the remaining runtime foundations. [Orca 50.2 dependency and configuration documentation](https://github.com/GNOME/orca/blob/50.2/README.md).

## Launch in an existing private graphical session

The runnable shell helper is `/tmp/canopa-orca-fixture/start-in-private-session.sh`. Source it **inside** the already-created private session shell:

```sh
source /tmp/canopa-orca-fixture/start-in-private-session.sh
```

It requires `XDG_RUNTIME_DIR` under `/tmp/canopa-*`, a `/tmp` Unix-path session bus and a Wayland display. It preserves the existing XDG config/data/state/cache/runtime locations and display. It adds the extracted Python, typelib, library, GSettings schema, eSpeak data and Speech Dispatcher plugin paths; it uses the keyfile GSettings backend in the fixture's config directory. Schemas were compiled with `glib-compile-schemas` in the extracted tree.

The helper starts `/usr/lib/at-spi-bus-launcher --launch-immediately --a11y=1` if the private session does not already own `org.a11y.Bus`, obtains and exports its `AT_SPI_BUS_ADDRESS`, and starts `/usr/lib/at-spi2-registryd` if needed. Explicit registry startup matters when a private bus has no systemd user activation manager. This lets already-running AccessKit adapters activate dynamically.

Speech Dispatcher receives explicit config, module, socket and PID-file paths. Relocation also needs `SPEECHD_PLUGIN_DIR`; specifying `--module-dir` alone does not relocate its audio plugins. `ESPEAK_DATA_PATH` points directly to `root/usr/share/espeak-ng-data`. These knobs were verified against the shipped binaries and exercised in the smoke test. [Speech Dispatcher configuration](https://github.com/brailcom/speechd/blob/0.12.1/config/speechd.conf), [eSpeak module configuration](https://github.com/brailcom/speechd/blob/0.12.1/config/modules/espeak-ng.conf).

Audio uses ALSA's file plugin with a null slave. It generates real synthesized PCM in the fixture's `orca-logs/speech.wav` and never selects PulseAudio/PipeWire or a hardware device. [ALSA file and null PCM plugins](https://www.alsa-project.org/alsa-doc/alsa-lib/pcm_plugins.html). Orca's separate sound effects and braille are disabled in fixture-local GSettings; speech remains enabled with the eSpeak module.

For a fresh session, the environment skeleton is:

```sh
canopa_fixture=$(mktemp -d /tmp/canopa-orca-desktop.XXXXXX)
mkdir -p "$canopa_fixture"/{runtime,config,data,state,cache}
chmod 700 "$canopa_fixture/runtime"
env -u AT_SPI_BUS_ADDRESS -u DISPLAY -u WAYLAND_DISPLAY -u SWAYSOCK \
  XDG_RUNTIME_DIR="$canopa_fixture/runtime" \
  XDG_CONFIG_HOME="$canopa_fixture/config" \
  XDG_DATA_HOME="$canopa_fixture/data" \
  XDG_STATE_HOME="$canopa_fixture/state" \
  XDG_CACHE_HOME="$canopa_fixture/cache" \
  dbus-run-session -- bash --noprofile --norc
```

Start the chosen isolated compositor inside that shell, select its Wayland socket, then source the helper. The tested product acceptance session used Niri inside Sway at `/tmp/canopa-completion-desktop/runtime/wayland-2`; the research helper does not start a compositor or reconnect to the live desktop. Private Unix-socket operations require execution outside the Codex filesystem/network sandbox on this machine. Running the speech process and client together avoids a tooling session ending the server between commands.

## Verified evidence and capture

`/tmp/canopa-orca-fixture/speech-smoke.py` started the relocated Speech Dispatcher, sent a phrase with `spd-say --wait`, and terminated its own server. Result: exit 0, 59,093 mono 16-bit frames at 22,050 Hz, 45,249 nonzero samples, peak 30,271. The finalized sample is `/tmp/canopa-orca-fixture/logs/speech-final.wav`. This proves real speech generation through the installed module API, not spoken usability of a notification.

The parent then sourced the helper in the existing graphical fixture. At 16:27:35, `orca-logs/speech-protocol.log` recorded client `unknown:Orca:default` and SSML for “Screen reader on.” The AT-SPI registry log confirmed registry ownership; initial Orca console log contained no errors. Notification-specific evidence is collected separately by the parent through actual product interactions.

ALSA's recorded WAV kept zero lengths in its RIFF/data header after the smoke process ended, despite containing valid PCM. The helper `/tmp/canopa-orca-fixture/finalize-wav.py SOURCE DESTINATION` validates PCM layout and writes a new WAV with correct lengths, preserving the original recording. Run it after stopping the fixture's speech process:

```sh
kill -TERM "$CANOPA_ORCA_PID"
wait "$CANOPA_ORCA_PID"
kill -TERM "$CANOPA_SPEECHD_PID"
wait "$CANOPA_SPEECHD_PID"
python3 /tmp/canopa-orca-fixture/finalize-wav.py \
  "$CANOPA_ORCA_LOGS/speech.wav" "$CANOPA_ORCA_LOGS/speech-final.wav"
```

These exported PIDs identify only processes launched by this helper; do not use global `pkill`, Orca `--replace`, or stop the compositor to stop speech. Preserve protocol/debug logs alongside the WAV. The source writes Orca debug messages through a buffered file without per-message flush, so an initially empty debug file is not proof of inactivity. The Speech Dispatcher protocol log exposes SSML as it arrives.

## Native accessible controls and reading

Verified against the extracted Orca 50.2 `flat_review_presenter.py`, `where_am_i_presenter.py`, `say_all_presenter.py`, `dbus_service.py` and `scripts/default.py`. The default keyboard layout is desktop, with NumLock off:

| Interaction | Command |
| --- | --- |
| Read focused name, role and context | Keypad Enter; double press for detail |
| Toggle flat review | Keypad minus |
| Read entire visible window through flat review | Keypad plus twice |
| Current / previous / next line | Keypad 8 / 7 / 9 |
| Current / previous / next item | Keypad 5 / 4 / 6 |
| Native action/link focus | Application Tab / Shift+Tab behavior |
| Activate focused action/link | Application Enter behavior |

Flat review needs correct component geometry and an active application; “Say All” of a text object does not automatically traverse every unrelated native control. Browser structural-navigation keys should not be assumed to work on native Alert/Window trees. An image accessible name supplies alternative text even when the visual decoded successfully.

If compositor key forwarding is unavailable, Orca's own remote controller can invoke the same reader commands on its current app context. This is real Orca reading, but it does not validate physical key delivery. Query `org.gnome.Orca.Service` at `/org/gnome/Orca/Service/FlatReviewPresenter`, interface `org.gnome.Orca.Module`, method `ListCommands`; call `ExecuteCommand "SayAll" true`. `WhereAmIPresenter` exposes `WhereAmIBasic` and `WhereAmIDetailed` through the same interface. [Upstream remote controller source](https://github.com/GNOME/orca/blob/50.2/src/orca/dbus_service.py), [flat review implementation](https://github.com/GNOME/orca/blob/50.2/src/orca/flat_review_presenter.py).

Orca's default `object:announcement` handler presents `event.any_data` directly. For one-announcement and no-Center-refresh-repeat evidence, send a unique body marker, count its actual SSML `SPEAK` records, refresh without changing content and compare. Keep focus-reading repetitions separate from unsolicited announcement counts. Action label, full-body order, successful-image alt text, replacement, dismissal and link interaction need their own recorded product assertions; startup speech alone proves none of them. [Default event handling](https://github.com/GNOME/orca/blob/50.2/src/orca/scripts/default.py).

## Findings from the first actual product pass

Read-only inspection of the parent's graphical fixture logs established these initial results:

- At 16:30:20–24, actual Orca SSML read the Center's clear/delete button labels and roles, Allow/Silence/Drop radio names and selection states, then “Read the report” and its link role after real Tab navigation.
- At 16:30:47, `FlatReviewPresenter.SayAll` read the body in order: “Before bold”, “Blue and gold square”, “After”, “Read the report”, “End”. This includes the alternative text of the successfully displayed image. It also read the surrounding Center controls and history metadata.
- `WhereAmIBasic` on the focused link raised a real interoperability error. The argument `true` was correct; the remote wrapper selected the active Niribar script. Orca reached `AXHypertext.get_link_uri`, obtained `None` from `Atspi.Accessible.get_hyperlink`, then passed it to `Atspi.Hyperlink.get_uri`, producing `TypeError`. The bridge had exposed the Link role without its URL. Cached `accesskit_atspi_common 0.20.0` exposes the Hyperlink interface only when the node supports a URL. The corrective product change is to populate `Node::set_url` with the validated link target on both surfaces, then retry actual Orca link detail reading. [AccessKit node interface selection](https://docs.rs/crate/accesskit_atspi_common/0.20.0/source/src/node.rs), [Orca hyperlink helper](https://github.com/GNOME/orca/blob/50.2/src/orca/ax_hypertext.py).
- The console also reported `Unknown signature so for RemoveAccessible` during retirements. Cached AccessKit Unix 0.23.0 passes a reference structure directly to `emit_signal`, whereas the public cache signal requires one `(so)` structure argument. This is a separate upstream wire-serialization issue to verify with a real D-Bus signal test; wrapping the value in a one-element argument tuple is the proposed fix. Verify `AddAccessible`'s single structure argument too. [AT-SPI cache XML](https://github.com/GNOME/at-spi2-core/blob/main/xml/Cache.xml), [AccessKit emitter](https://docs.rs/crate/accesskit_unix/0.23.0/source/src/atspi/bus.rs).

The findings above describe the first pass before the parent's corrective edits. They are not claims that the issues remain in the final implementation. `/tmp/canopa-orca-fixture/speech-transcript.py LOG` extracts actual SSML submissions with their source line and timestamp for subsequent evidence comparison; it does not synthesize messages or interact with any bus.

## Bounded correction: real hyperlink destination metadata

Carry the provider's validated HTTP(S) destination in generic panel presentation metadata and set that value on the native accessibility node. Retain the existing opaque, revision-safe activation command: reading destination metadata must not cause the host to open a URI independently. ADR 0050 should distinguish these two responsibilities. A provider without destination metadata can still supply an ordinary action; the host should not manufacture a destination to give it hyperlink semantics.

AccessKit 0.25.0 documents `Node::set_url` as the target URL of a link. In `accesskit_atspi_common 0.20.0`, URL presence controls the Hyperlink interface, `NAnchors` is one when a URL exists, `IsValid` uses the same presence test, and `GetURI(0)` returns the stored value. Therefore `set_url("")` would expose a supposedly valid one-anchor hyperlink whose destination is empty. It would get past the observed Orca `None` failure but provide no meaningful destination for link detail reading. This is a recommendation from the implementation's semantics and the actual Orca failure; the research did not establish a separate general AT-SPI prohibition on every empty URI. [AccessKit URL property](https://docs.rs/accesskit/0.25.0/accesskit/struct.Node.html#method.set_url), [AT-SPI adapter implementation](https://docs.rs/crate/accesskit_atspi_common/0.20.0/source/src/node.rs).

The observed `WhereAmIBasic` error is unrelated to the remote command boolean: Orca successfully selected the active Niribar script and followed the link-reading path before receiving the missing Hyperlink interface. After the destination correction, rerun both `WhereAmIBasic` and `WhereAmIDetailed` while that link retains focus, then deliberately activate it and verify the existing provider-owned portal request. The successful flat-review body/alternative-text reading above remains separate evidence.

## Bounded correction: AT-SPI cache signal arguments

The public cache interface requires exactly one structure argument for each signal:

| Signal | Required D-Bus body signature |
| --- | --- |
| `AddAccessible` | `((so)(so)(so)iiassusau)` |
| `RemoveAccessible` | `(so)` |

The add structure is one element of the `GetItems` result, whose signature is `a((so)(so)(so)iiassusau)`. [Upstream AT-SPI cache interface](https://github.com/GNOME/at-spi2-core/blob/main/xml/Cache.xml).

Cached `accesskit_unix 0.23.0`, `src/atspi/bus.rs`, currently sends each structure directly as the zbus message body. The smallest proposed source patch wraps each value in the tuple of method/signal arguments:

```rust
self.emit_cache_signal("AddAccessible", &(item,)).await
// ...
self.emit_cache_signal("RemoveAccessible", &(reference,)).await
```

An equivalent alternative is to leave both callers alone and pass `&(body,)` to `Connection::emit_signal` inside `emit_cache_signal`; that helper has only these two callers. Apply one alternative, not both. Do not change the separate event-emission helper, whose body has different semantics. Confirm the emitted message header signatures on a private bus and decode each as a one-element tuple; a tree-only unit test cannot catch this wire defect. Then retire/update real nodes with Orca running and check that new console output contains no cache signature warning. [Release emitter source](https://docs.rs/crate/accesskit_unix/0.23.0/source/src/atspi/bus.rs).

## AccessKit Unix vendoring provenance

The locally cached `accesskit_unix-0.23.0.crate` identifies its source revision in `.cargo_vcs_info.json` as `42e53b0d829e7a0b34dc8803bb06012db2e80cc6`, path `adapters/unix`. Its normalized Cargo metadata declares `MIT OR Apache-2.0`, author `The AccessKit contributors`, and repository `https://github.com/AccessKit/accesskit`. These are verified release-artifact facts; no release tag was inferred.

| Artifact | Verified SHA-256 |
| --- | --- |
| Cached `accesskit_unix-0.23.0.crate` | `665c9079b7325a8168a6793437a172dda44b19a05af8ed52d7e32abaada996fe` |
| Exact-revision `LICENSE-MIT` | `23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3` |
| Exact-revision `LICENSE-APACHE` | `62c7a1e35f56406896d7aa7ca52d0cc0d272ac022b5d2796e7d6905db8a3636a` |

The crate archive has no license text files. Both upstream root license files were successfully downloaded from the recorded revision to `/tmp/accesskit-42e53b0d-LICENSE-MIT` and `/tmp/accesskit-42e53b0d-LICENSE-APACHE`. The MIT file has no separate copyright header; preserve it verbatim together with existing source notices rather than inventing a copyright holder or year. For example, the affected emitter's source already credits the AccessKit Authors with year 2022. Retain the upstream dual-license metadata and record the narrow local cache-signal patch in vendor provenance. [Exact-revision MIT text](https://raw.githubusercontent.com/AccessKit/accesskit/42e53b0d829e7a0b34dc8803bb06012db2e80cc6/LICENSE-MIT), [exact-revision Apache text](https://raw.githubusercontent.com/AccessKit/accesskit/42e53b0d829e7a0b34dc8803bb06012db2e80cc6/LICENSE-APACHE), [recorded source revision](https://github.com/AccessKit/accesskit/tree/42e53b0d829e7a0b34dc8803bb06012db2e80cc6/adapters/unix).

Local cache locations are rooted at `/home/kevinard/.local/share/cargo/registry/`: the archive is `cache/index.crates.io-1949cf8c6b5b557f/accesskit_unix-0.23.0.crate`, and unpacked source is `src/index.crates.io-1949cf8c6b5b557f/accesskit_unix-0.23.0`. This investigation modifies documentation only; the parent owns the product patch, final public-bus regression, and final Orca acceptance evidence.

## Keyboard-driven reader commands in the nested fixture

The installed compositor versions are Niri `26.04 (8ed0da4)` and Sway `1.12`. The private Niri log confirms it loaded `/tmp/canopa-completion-desktop/niri.kdl`, including subsequent reloads. Its F6/F7 bindings use valid `spawn` argument separation. Changing F6 to Alt+F6 or Super+F6 cannot fix the relevant input-path limitation: Niri does not route `zwp_virtual_keyboard_v1` events through its compositor binding handler in this release. This exactly matches upstream issue 403 and its duplicate 2212: applications receive virtual typing while compositor shortcuts do not fire. As checked on 2026-09-16, the proposed upstream fix is still open and depends on a Smithay change. [Original Niri issue](https://github.com/niri-wm/niri/issues/403), [duplicate report](https://github.com/niri-wm/niri/issues/2212), [proposed fix 4548](https://github.com/niri-wm/niri/pull/4548).

The release source downloaded for this investigation is `/tmp/niri-26.04`, from `https://codeload.github.com/niri-wm/niri/tar.gz/refs/tags/v26.04`. `src/handlers/mod.rs` delegates virtual keyboard handling to Smithay, while `src/backend/winit.rs` sends nested window input through `process_input_event`. The latter reaches `src/input/mod.rs::on_keyboard`, which handles both accessibility grabs and compositor binds. The release's Cargo lock pins Smithay to `ff5fa7df392cecfba049ffed55cdaa4e98a8e7ef`. This explains why sending keys directly to Niri's virtual keyboard interface differs from ordinary keyboard input to its outer window. [Niri release handlers](https://github.com/niri-wm/niri/blob/v26.04/src/handlers/mod.rs), [nested backend](https://github.com/niri-wm/niri/blob/v26.04/src/backend/winit.rs), [keyboard input path](https://github.com/niri-wm/niri/blob/v26.04/src/input/mod.rs).

### Selected acceptance route: private outer Sway shortcuts

Add these bindings to the **outer private Sway** fixture config, retaining the existing Niri/Center focus. Sway's keyboard path evaluates bindings using the virtual device's keymap, so `wtype` can trigger them. Its source attaches virtual keyboards to a Sway seat and evaluates keysym bindings in the normal keyboard handler. Installed Sway's manual confirms `--no-repeat` avoids repeated invocation while a key is held. [Sway virtual keyboard attachment](https://github.com/swaywm/sway/blob/master/sway/input/input-manager.c), [Sway keyboard binding evaluation](https://github.com/swaywm/sway/blob/master/sway/input/keyboard.c).

```text
bindsym --no-repeat F6 exec --no-startup-id gdbus call --session --dest org.gnome.Orca.Service --object-path /org/gnome/Orca/Service/FlatReviewPresenter --method org.gnome.Orca.Module.ExecuteCommand SayAll true
bindsym --no-repeat F7 exec --no-startup-id gdbus call --session --dest org.gnome.Orca.Service --object-path /org/gnome/Orca/Service/WhereAmIPresenter --method org.gnome.Orca.Module.ExecuteCommand WhereAmIBasic true
```

Sway must spawn these commands with the private fixture's session bus address. If its startup environment differs, use an explicit `env DBUS_SESSION_BUS_ADDRESS=...` prefix containing that exact private address. Reload only the outer fixture using its explicit Sway IPC socket. Then inject one key at a time into its saved outer Wayland socket:

```sh
test -n "$OUTER_WAYLAND_DISPLAY"
env WAYLAND_DISPLAY="$OUTER_WAYLAND_DISPLAY" wtype -k F6
# After reading completes, refocus the desired Center link/control as necessary.
env WAYLAND_DISPLAY="$OUTER_WAYLAND_DISPLAY" wtype -k F7
```

The shell retains the fixture's private `XDG_RUNTIME_DIR`; `OUTER_WAYLAND_DISPLAY` identifies its Sway display, not Niri's `wayland-2` or the live desktop. Record the chosen socket, injected key, compositor command result, Orca command/debug event and resulting SSML. Require the ordered body and successful-image alternative text after F6, and the focused label/role/link information after F7. Orca's remote controller invokes its real presenter in the active application context. [Orca remote controller](https://github.com/GNOME/orca/blob/50.2/src/orca/dbus_service.py).

This is keyboard-driven acceptance through a configured compositor shortcut: virtual keyboard event → Sway binding → Orca presenter → AT-SPI reading and synthesized speech. It does **not** establish physical hardware input, Niri's direct virtual-keyboard binding support, or Orca's default keypad/global keyboard grabs. Those distinctions must remain explicit in final acceptance evidence. Application Tab/Shift+Tab/Enter behavior remains separately tested through the product's own keyboard handling.

### Separate prerequisite for default Orca keypad commands

Niri creates `org.freedesktop.a11y.KeyboardMonitor` only for a session instance or when startup config enables `debug { dbus-interfaces-in-non-session-instances; }`. The fixture's inspected config lacks that debug flag. Its creation is in `DBusServers::start`, so adding it by ordinary live config reload is insufficient to establish the service. A fresh isolated Niri startup with this option and compatible ordinary nested keyboard input is a separate way to test Orca's native grabs. `on_keyboard` processes the a11y monitor before normal bindings; Orca 50.2 creates its AT-SPI device with `Atspi.Device.new_full("org.gnome.Orca")`. [Niri D-Bus startup](https://github.com/niri-wm/niri/blob/v26.04/src/dbus/mod.rs), [Niri a11y monitor](https://github.com/niri-wm/niri/blob/v26.04/src/dbus/freedesktop_a11y.rs), [Orca AT-SPI device setup](https://github.com/GNOME/orca/blob/50.2/src/orca/ax_device_manager.py).

Simply changing `wtype`'s destination to outer Sway and forwarding its events into Niri is not a reliable default-keypad test: wtype assigns sequential synthetic keycodes to the requested symbols, whereas the nested input route interprets physical codes. Cached Winit 0.30.13 derives the physical key from the raw keycode in `src/platform_impl/linux/common/xkb/mod.rs` and `keymap.rs`. [wtype keymap construction](https://github.com/atx/wtype/blob/master/main.c), [Winit keycode mapping](https://github.com/rust-windowing/winit/blob/v0.30.13/src/platform_impl/linux/common/xkb/keymap.rs). A future confined sender would need a complete compatible evdev keymap and correct event codes. That helper was deliberately left unimplemented after selecting the simpler outer-Sway shortcut route; no uinput device or live/private desktop process was changed by this research.
