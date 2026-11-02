"""Native media input, real decoded video pixels and isolated recorded audio."""
import array
import json
import math
import os
from pathlib import Path
import struct
import time
import wave

AUDIO_FORMATS = (("A-mp3.mp3", "libmp3lame", 440),
                 ("B-flac.flac", "flac", 550),
                 ("C-opus.opus", "libopus", 660),
                 ("D-aac.m4a", "aac", 770))


def format_fixtures(session, fixture):
    folder = fixture / "Media formats"
    folder.mkdir()
    for name, codec, frequency in AUDIO_FORMATS:
        session.run(["ffmpeg", "-v", "error", "-filter_complex_threads", "1", "-f", "lavfi", "-i",
                     f"sine=frequency={frequency}:sample_rate=48000:duration=10",
                     "-f", "lavfi", "-i", f"sine=frequency={frequency + 220}:sample_rate=48000:duration=20",
                     "-filter_complex", "[0:a][1:a]concat=n=2:v=0:a=1[a]", "-map", "[a]",
                     "-c:a", codec, str(folder / name)])
    session.run(["ffmpeg", "-v", "error", "-filter_complex_threads", "1", "-f", "lavfi", "-i", "color=c=red:s=320x240:r=10:d=4",
                 "-f", "lavfi", "-i", "color=c=green:s=320x240:r=10:d=4",
                 "-f", "lavfi", "-i", "color=c=blue:s=320x240:r=10:d=4",
                 "-f", "lavfi", "-i", "sine=frequency=990:sample_rate=48000:duration=12",
                 "-filter_complex", "[0:v][1:v][2:v]concat=n=3:v=1:a=0[v]", "-map", "[v]", "-map", "3:a",
                 "-c:a", "aac", "-c:v", "libx264", "-preset", "ultrafast", "-threads", "1",
                 "-pix_fmt", "yuv420p", "-movflags", "+faststart", str(folder / "E-video.mp4")])


def tone(path, seconds, frequency=440):
    with wave.open(str(path), "wb") as stream:
        stream.setparams((1, 2, 48000, 0, "NONE", "not compressed"))
        stream.writeframes(b"".join(struct.pack("<h", round(6000 * math.sin(2 * math.pi * frequency * sample / 48000)))
                                    for sample in range(round(seconds * 48000))))


def setup(session, fixture, formats_only=False, accessibility=True):
    audio = session.runtime / "audio"
    audio.mkdir()
    # Same isolated protocol setup as the shell fixture, with a dummy clock and
    # policy-only WirePlumber for actual streams. No hardware monitor is loaded.
    session.env.update(PIPEWIRE_RUNTIME_DIR=str(audio), PIPEWIRE_CONFIG_DIR=str(audio),
                       PIPEWIRE_CONFIG_PREFIX="", PIPEWIRE_REMOTE="pipewire-0",
                       PULSE_RUNTIME_PATH=str(audio / "pulse"), PULSE_SERVER=f"unix:{audio}/pulse/native",
                       ALSA_CONFIG_PATH=str(audio / "alsa.conf"), SDL_AUDIODRIVER="pulseaudio",
                       GTK_A11Y="atspi", LANG="C.UTF-8")
    (audio / "alsa.conf").write_text('pcm.!default { type null }\n')
    (audio / "client.conf").write_text('context.modules = [ { name = libpipewire-module-protocol-native } { name = libpipewire-module-metadata } ]')
    core = '''context.properties = { core.daemon = true core.name = pipewire-0 }
      context.spa-libs = { audio.convert.* = audioconvert/libspa-audioconvert support.* = support/libspa-support }
      context.modules = [
        { name = libpipewire-module-protocol-native } { name = libpipewire-module-access }
        { name = libpipewire-module-metadata } { name = libpipewire-module-spa-node-factory }
        { name = libpipewire-module-client-node } { name = libpipewire-module-adapter }
        { name = libpipewire-module-link-factory }
      ]
      context.objects = [
        { factory = metadata args = { metadata.name = default } }
        { factory = spa-node-factory args = { factory.name = support.node.driver node.name = Fixture-Driver priority.driver = 20000 node.group = pipewire.dummy } }
      ]'''
    pulse = '''context.spa-libs = { audio.convert.* = audioconvert/libspa-audioconvert support.* = support/libspa-support }
      context.modules = [
        { name = libpipewire-module-protocol-native } { name = libpipewire-module-client-node }
        { name = libpipewire-module-adapter } { name = libpipewire-module-metadata }
        { name = libpipewire-module-protocol-pulse args = { server.address = [ "unix:native" ] } }
      ]'''
    for program, config, socket in [("pipewire", core, "pipewire-0"), ("pipewire-pulse", pulse, "pulse/native")]:
        (audio / f"{program}.conf").write_text(config)
        session.start([program, "-c", f"{program}.conf"], f"{program}.log")
        session.wait_for(lambda: (audio / socket).exists(), f"private {program}")
    session.start(["wireplumber", "--profile", "policy"], "wireplumber.log")
    session.run(["pactl", "load-module", "module-null-sink", "sink_name=fixture-output", "sink_properties=device.description=FixtureOutput"])
    session.run(["pactl", "set-default-sink", "fixture-output"])
    session.run(["pactl", "set-sink-volume", "fixture-output", "100%"])
    session.start(["parec", "--raw", "--format=s16le", "--rate=48000", "--channels=1",
                   "--latency-msec=20", "--device=fixture-output.monitor", str(session.output / "captured.pcm")], "parec.log")
    media = fixture / "Media"
    media.mkdir()
    tone(media / "A-audio.wav", 60)
    tone(audio / "calibration.wav", .3)
    session.run(["paplay", str(audio / "calibration.wav")])
    (media / "C-text.txt").write_text("The preview keeps its browser's selection.\n")
    (media / "D-broken.webm").write_bytes(b"Invalid media for a visible playback error.\n")
    session.run(["ffmpeg", "-v", "error", "-filter_complex_threads", "1", "-f", "lavfi", "-i", "color=c=red:s=320x240:r=10:d=6",
                 "-f", "lavfi", "-i", "color=c=green:s=320x240:r=10:d=6",
                 "-f", "lavfi", "-i", "color=c=blue:s=320x240:r=10:d=6",
                 "-f", "lavfi", "-i", "sine=frequency=660:sample_rate=48000:duration=18",
                 "-filter_complex", "[0:v][1:v][2:v]concat=n=3:v=1:a=0[v]", "-map", "[v]",
                 "-map", "3:a", "-c:a", "libvorbis", "-c:v", "libvpx", "-threads", "1", "-deadline", "realtime", "-cpu-used", "8", str(media / "B-video.webm")])
    if formats_only:
        format_fixtures(session, fixture)
    if not accessibility:
        return
    # All accessibility clients/providers use this fixture's private buses.
    os.environ["DBUS_SESSION_BUS_ADDRESS"] = session.env["DBUS_SESSION_BUS_ADDRESS"]
    import gi
    gi.require_version("Gio", "2.0")
    from gi.repository import Gio, GLib
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    def call(destination, path, interface, method, parameters=None):
        return bus.call_sync(destination, path, interface, method, parameters, None,
                             Gio.DBusCallFlags.NONE, 5000, None)
    roots = list(map(Path, ("/usr/lib", "/usr/libexec", "/usr/lib/at-spi2-core")))
    def executable(name):
        return str(next(root / name for root in roots if (root / name).is_file()))
    session.env["GSETTINGS_BACKEND"] = "memory"
    session.start([executable("at-spi-bus-launcher"), "--launch-immediately"], "a11y-launcher.log")
    session.wait_for(lambda: call("org.freedesktop.DBus", "/org/freedesktop/DBus", "org.freedesktop.DBus",
                                 "NameHasOwner", GLib.Variant("(s)", ("org.a11y.Bus",))).unpack()[0], "private accessibility bus")
    call("org.a11y.Bus", "/org/a11y/bus", "org.freedesktop.DBus.Properties",
         "Set", GLib.Variant("(ssv)", ("org.a11y.Status", "IsEnabled", GLib.Variant("b", True))))
    address = call("org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus", "GetAddress").unpack()[0]
    os.environ["AT_SPI_BUS_ADDRESS"] = address
    session.env["AT_SPI_BUS_ADDRESS"] = address
    session.start([executable("at-spi2-registryd")], "a11y-registry.log")


def verify(session, fixture, scheme, scale, formats_only=False):
    import gi
    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi, GLib

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(value):
        return next((window for window in windows() if window.get("title") == value), None)

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name]); time.sleep(.25)

    def ctrl(key):
        session.run(["wtype", "-M", "ctrl", "-k", key, "-m", "ctrl"]); time.sleep(.3)

    def find(name, node=None, depth=0, focused_only=False):
        if depth == 0:
            context = GLib.MainContext.default()
            while context.pending(): context.iteration(False)
        node = node or Atspi.get_desktop(0)
        if depth > 24: return None
        try:
            if node.get_name() == name and (not focused_only or node.get_state_set().contains(Atspi.StateType.FOCUSED)): return node
            for index in range(min(node.get_child_count(), 256)):
                match = find(name, node.get_child_at_index(index), depth + 1, focused_only)
                if match: return match
        except GLib.Error:
            pass
        return None

    def focus_control(*names):
        for _ in range(30):
            for name in names:
                control = find(name, focused_only=True)
                if control: return control
            keys("Tab")
        raise AssertionError(f"Tab cannot focus {names}")

    recording = session.output / "captured.pcm"
    session.wait_for(lambda: recording.exists() and recording.stat().st_size > 24000, "actual monitor PCM")
    calibration = array.array("h", recording.read_bytes())
    assert max(map(abs, calibration), default=0) > 1000, "private audio fixture did not deliver its calibration signal"

    def samples(seconds=.2):
        start = recording.stat().st_size
        count = round(seconds * 48000) * 2
        session.wait_for(lambda: recording.stat().st_size >= start + count, "fresh PCM samples")
        with recording.open("rb") as stream:
            stream.seek(start); data = stream.read(count)
        return array.array("h", data)

    def frame_color(name, expected, window_title="B-video.webm — Quick Look"):
        frame = session.capture(name)
        window = title(window_title)
        x, y = window["layout"]["tile_pos_in_workspace_view"]
        width, height = window["layout"]["window_size"]
        pixel = frame.getpixel((round((x + width / 2) * scale), round((y + height / 2) * scale)))
        return all(abs(a - b) < 20 for a, b in zip(pixel, expected))

    if formats_only:
        measurements = []
        def decoded_tone(expected):
            signal = samples(.1)
            frequency = sum(left <= 0 < right for left, right in zip(signal, signal[1:])) * 48000 / len(signal)
            measurements.append({"expected_hz": expected, "measured_hz": frequency,
                                 "peak": max(map(abs, signal), default=0)})
            (session.output / "format-tones.json").write_text(json.dumps(measurements, indent=2))
            return abs(frequency - expected) < 25 and measurements[-1]["peak"] > 1000

        session.run(["niri", "msg", "action", "set-column-width", "360"])
        ctrl("l"); session.run(["wtype", str(fixture / "Media formats")]); keys("Return")
        session.wait_for(lambda: title("Media formats — Canopa Files") and find("A-mp3.mp3"), "rendered format folder")
        keys("Home")
        checks = ["private media calibration delivers real PCM"]
        for name, _, frequency in AUDIO_FORMATS:
            keys("space")
            session.wait_for(lambda: title(f"{name} — Quick Look") and find("Play"), f"prepared {name} preview")
            if name == "A-mp3.mp3":
                for iteration in range(8):
                    keys("Escape")
                    session.wait_for(lambda: title(f"{name} — Quick Look") is None, "tagged MP3 closes before preparation stress")
                    keys("space")
                    session.wait_for(lambda: title(f"{name} — Quick Look") and find("Play"), f"tagged MP3 preparation {iteration + 2}")
                checks.append("tagged MP3 repeatedly opens without aborting")
            time.sleep(.3)
            assert max(map(abs, samples(.1)), default=0) == 0, f"{name} autoplayed"
            focus_control("Play"); keys("space")
            session.wait_for(lambda: decoded_tone(frequency), f"{name} first decoded tone")
            focus_control("Stop", "Pause"); keys("space")
            session.wait_for(lambda: max(map(abs, samples(.1)), default=0) == 0, f"{name} paused audio", timeout=5)
            position = focus_control("Position")
            keys("End", "Left", "Left", "Left", "Left")
            value = position.get_value_iface()
            assert value.get_maximum_value() * .5 < value.get_current_value() < value.get_maximum_value(), f"{name} position did not move into its second tone"
            focus_control("Play"); keys("space")
            session.wait_for(lambda: decoded_tone(frequency + 220), f"{name} actual seek into second tone", timeout=5)
            focus_control("Stop", "Pause"); keys("space")
            session.wait_for(lambda: max(map(abs, samples(.1)), default=0) == 0, f"{name} paused after seek", timeout=5)
            focus_control("Position"); keys("Home")
            focus_control("Play"); keys("space")
            session.wait_for(lambda: decoded_tone(frequency), f"{name} actual rewind to first tone", timeout=5)
            session.capture(f"format-{name}.png")
            keys("Escape")
            session.wait_for(lambda: title(f"{name} — Quick Look") is None, f"{name} preview closes")
            session.wait_for(lambda: max(map(abs, samples(.1)), default=0) == 0, f"{name} close stops audio", timeout=5)
            checks.extend([f"{name} does not autoplay", f"{name} plays its actual tone", f"{name} Pause stops PCM",
                           f"{name} seeks across distinct tones and rewinds", f"{name} close stops PCM"])
            keys("Down")
        keys("space")
        video_title = "E-video.mp4 — Quick Look"
        session.wait_for(lambda: title(video_title) and find("Play"), "prepared H.264/AAC MP4")
        time.sleep(.3)
        assert max(map(abs, samples(.1)), default=0) == 0, "MP4 autoplayed"
        focus_control("Play"); keys("space")
        session.wait_for(lambda: decoded_tone(990), "actual MP4 AAC audio")
        session.wait_for(lambda: frame_color("format-mp4-red.png", (255, 0, 0), video_title), "decoded H.264 red frame", timeout=5)
        session.wait_for(lambda: frame_color("format-mp4-green.png", (0, 128, 0), video_title), "advancing H.264 green frame", timeout=8)
        focus_control("Stop", "Pause"); keys("space")
        session.wait_for(lambda: max(map(abs, samples(.1)), default=0) == 0, "MP4 pause stops AAC audio", timeout=5)
        focus_control("Position"); keys("Home")
        session.wait_for(lambda: frame_color("format-mp4-rewound.png", (255, 0, 0), video_title), "paused MP4 rewind renders red", timeout=5)
        assert max(map(abs, samples(.1)), default=0) == 0, "MP4 paused seek resumed audio"
        keys("Escape")
        session.wait_for(lambda: title(video_title) is None, "MP4 closes")
        checks.extend(["MP4 does not autoplay", "MP4 AAC track produces its distinct tone", "H.264 frames advance",
                       "MP4 Pause stops PCM", "paused MP4 rewind changes the rendered frame without playing"])
        browser = title("Media formats — Canopa Files")
        session.run(["niri", "msg", "action", "close-window", "--id", str(browser["id"])])
        session.wait_for(lambda: title("Media formats — Canopa Files") is None, "format browser closes")
        return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks, "tones": measurements}

    session.run(["niri", "msg", "action", "set-column-width", "360"])
    ctrl("l"); session.run(["wtype", str(fixture / "Media")]); keys("Return")
    session.wait_for(lambda: title("Media — Canopa Files"), "media browser")
    keys("Home", "space")
    preview = session.wait_for(lambda: title("A-audio.wav — Quick Look"), "audio preview")
    assert preview["is_floating"] and preview["layout"]["window_size"][0] >= 600
    play = session.wait_for(lambda: find("Play"), "native media Play control")
    time.sleep(.5)
    assert max(map(abs, samples()), default=0) == 0, "audio preview autoplayed"
    session.capture("media-01-audio-paused.png")
    focus_control("Play")
    keys("space")
    assert title("A-audio.wav — Quick Look"), "Space on the focused Play control dismissed Quick Look"
    session.wait_for(lambda: max(map(abs, samples(.1)), default=0) > 1000, "actual decoded audio from the native Play control")
    session.capture("media-02-audio-playing.png")
    def describe(node, depth=0):
        if not node or depth > 18: return None
        try:
            return {"name": node.get_name(), "role": node.get_role_name(),
                    "focused": node.get_state_set().contains(Atspi.StateType.FOCUSED),
                    "focusable": node.get_state_set().contains(Atspi.StateType.FOCUSABLE),
                    "showing": node.get_state_set().contains(Atspi.StateType.SHOWING),
                    "children": [describe(node.get_child_at_index(index), depth + 1)
                                 for index in range(min(node.get_child_count(), 128))]}
        except GLib.Error: return None
    (session.output / "media-accessibility.json").write_text(json.dumps(describe(find("A-audio.wav — Quick Look")), indent=2))
    keys("space")
    session.wait_for(lambda: find("Play"), "stopped native playback control")
    session.wait_for(lambda: max(map(abs, samples(.1)), default=0) == 0, "Pause stops actual PCM", timeout=5)

    def seek_control(node=None, depth=0):
        node = node or find("A-audio.wav — Quick Look")
        if not node or depth > 24: return None
        try:
            if node.get_role() == Atspi.Role.SLIDER:
                value = node.get_value_iface()
                if value and value.get_maximum_value() > 5: return node
            for index in range(min(node.get_child_count(), 256)):
                match = seek_control(node.get_child_at_index(index), depth + 1)
                if match: return match
        except GLib.Error:
            pass
        return None

    seek = session.wait_for(seek_control, "native playback position slider")
    for _ in range(20):
        if seek.get_state_set().contains(Atspi.StateType.FOCUSED): break
        keys("Tab")
    else: raise AssertionError("Tab cannot reach the playback position slider")
    before = seek.get_value_iface().get_current_value()
    keys("Right")
    after = seek.get_value_iface().get_current_value()
    assert title("A-audio.wav — Quick Look") and after > before, "Right on the focused seek slider navigated the browser instead of seeking"
    session.capture("media-03-audio-seek.png")
    focus_control("Play"); keys("space")
    session.wait_for(lambda: max(map(abs, samples(.1)), default=0) > 1000, "audio resumes before changing selection")
    focus_control("Volume"); keys("space")

    def volume_control(node=None, depth=0):
        node = node or Atspi.get_desktop(0)
        if depth > 24: return None
        try:
            if node.get_role() == Atspi.Role.SLIDER and node.get_state_set().contains(Atspi.StateType.SHOWING):
                value = node.get_value_iface()
                if value and 0 < value.get_maximum_value() <= 1: return node
            for index in range(min(node.get_child_count(), 256)):
                match = volume_control(node.get_child_at_index(index), depth + 1)
                if match: return match
        except GLib.Error: pass
        return None

    volume = session.wait_for(volume_control, "native volume popover slider")
    for _ in range(20):
        if volume.get_state_set().contains(Atspi.StateType.FOCUSED): break
        keys("Tab")
    else: raise AssertionError("Tab cannot focus the volume slider")
    keys("Home")
    session.wait_for(lambda: max(map(abs, samples(.1)), default=0) == 0, "volume Home mutes actual audio", timeout=5)
    volume_values = [{"key": "Home", "value": volume.get_value_iface().get_current_value(), "focused": volume.get_state_set().contains(Atspi.StateType.FOCUSED)}]
    for _ in range(30):
        keys("Up")
        value = volume.get_value_iface().get_current_value()
        volume_values.append({"key": "Up", "value": value, "focused": volume.get_state_set().contains(Atspi.StateType.FOCUSED)})
        if value >= .95: break
    (session.output / "volume-values.json").write_text(json.dumps(volume_values, indent=2))
    session.wait_for(lambda: max(map(abs, samples(.1)), default=0) > 50, "volume Up restores actual audio", timeout=5)
    keys("Escape")
    assert title("A-audio.wav — Quick Look"), "Escape from volume controls closed Quick Look instead of the popover"
    session.capture("media-volume-restored.png")
    browser = title("Media — Canopa Files")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
    time.sleep(.3); keys("Down")
    video = session.wait_for(lambda: title("B-video.webm — Quick Look"), "source-bound video preview")
    assert title("Media — Canopa Files")["is_focused"], "preview loading stole focus from its browser"
    session.wait_for(lambda: max(map(abs, samples(.1)), default=0) == 0, "selection change stops the previous stream", timeout=5)
    assert max(map(abs, samples(.2)), default=0) == 0, "video preview autoplayed"
    session.run(["niri", "msg", "action", "focus-window", "--id", str(video["id"])])
    time.sleep(.3)
    session.wait_for(lambda: find("Play"), "native video Play control")
    (session.output / "video-accessibility.json").write_text(json.dumps(describe(find("B-video.webm — Quick Look")), indent=2))
    focus_control("Play"); keys("space")
    session.wait_for(lambda: max(map(abs, samples(.1)), default=0) > 1000, "decoded video audio")
    signal = samples(.1)
    frequency = sum(left <= 0 < right for left, right in zip(signal, signal[1:])) * 48000 / len(signal)
    assert 635 < frequency < 685, f"video emitted another stream's audio: {frequency} Hz"

    session.wait_for(lambda: frame_color("media-04-video-red.png", (255, 0, 0)), "actual decoded red video frame", timeout=5)
    session.wait_for(lambda: frame_color("media-05-video-green.png", (0, 128, 0)), "actual advancing green video frame", timeout=10)
    focus_control("Stop", "Pause"); keys("space")
    session.wait_for(lambda: max(map(abs, samples(.1)), default=0) == 0, "video Pause stops its audio", timeout=5)
    focus_control("Position"); keys("Home")
    session.wait_for(lambda: frame_color("media-video-rewound.png", (255, 0, 0)), "paused video seek renders the requested frame", timeout=5)
    assert max(map(abs, samples(.1)), default=0) == 0, "paused video seek resumed its audio"
    keys("Escape")
    session.wait_for(lambda: title("B-video.webm — Quick Look") is None, "media preview closes")
    assert title("Media — Canopa Files")["is_focused"]
    keys("space")
    session.wait_for(lambda: title("B-video.webm — Quick Look"), "video preview reopens")
    time.sleep(.4); keys("space")
    session.wait_for(lambda: title("B-video.webm — Quick Look") is None, "Space dismisses default preview focus")
    keys("space")
    session.wait_for(lambda: title("B-video.webm — Quick Look"), "video for control replacement")
    time.sleep(.4); focus_control("Play")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(browser["id"])])
    time.sleep(.3); keys("Down")
    text_preview = session.wait_for(lambda: title("C-text.txt — Quick Look"), "text replaces the focused media controls")
    session.run(["niri", "msg", "action", "focus-window", "--id", str(text_preview["id"])])
    time.sleep(.3); keys("space")
    session.wait_for(lambda: title("C-text.txt — Quick Look") is None, "replacing controls restores Space-to-close")
    keys("space")
    session.wait_for(lambda: title("C-text.txt — Quick Look"), "text preview for narrow parent navigation")
    time.sleep(.4); keys("Left")
    session.wait_for(lambda: title("Downloads — Canopa Files") and title("Media — Quick Look"), "Left from preview keeps its narrow parent-folder meaning")
    keys("Right")
    session.wait_for(lambda: title("Media — Canopa Files") and title("C-text.txt — Quick Look"), "Right returns with source selection preserved")
    keys("Escape", "End", "space")
    session.wait_for(lambda: title("D-broken.webm — Quick Look"), "corrupt media preview")

    def media_error(node=None, depth=0):
        node = node or find("D-broken.webm — Quick Look")
        if not node or depth > 24: return None
        try:
            if node.get_role() == Atspi.Role.LABEL and node.get_state_set().contains(Atspi.StateType.SHOWING):
                name = node.get_name()
                if any(word in name.lower() for word in ("could", "cannot", "unable", "unsupported", "missing", "failed")): return name
            for index in range(min(node.get_child_count(), 256)):
                match = media_error(node.get_child_at_index(index), depth + 1)
                if match: return match
        except GLib.Error: pass
        return None

    error = session.wait_for(media_error, "visible explanation for corrupt media")
    session.capture("media-error.png")
    keys("Escape")
    keys("Home", "space")
    session.wait_for(lambda: title("A-audio.wav — Quick Look"), "audio preview for owner lifetime")
    time.sleep(.4); focus_control("Play"); keys("space")
    session.wait_for(lambda: max(map(abs, samples(.1)), default=0) > 1000, "audio plays before another browser opens")
    ctrl("n")
    other = session.wait_for(lambda: next((window for window in windows()
                                          if window["title"] == "Media — Canopa Files" and window["id"] != browser["id"]), None), "independent browser beside playing preview")
    assert title("A-audio.wav — Quick Look")
    assert max(map(abs, samples(.1)), default=0) > 1000, "another browser's focus stopped the source's stream"
    session.run(["niri", "msg", "action", "close-window", "--id", str(browser["id"])])
    session.wait_for(lambda: title("A-audio.wav — Quick Look") is None, "owner close destroys its floating preview")
    session.wait_for(lambda: max(map(abs, samples(.1)), default=0) == 0, "owner close stops actual audio", timeout=5)
    assert any(window["id"] == other["id"] for window in windows()), "owner close destroyed the independent browser"
    return {"status": "passed", "scheme": scheme, "scale": scale,
            "checks": ["private audio backend delivers real PCM", "audio preview does not autoplay", "focused Play receives Space and emits decoded audio", "Pause stops actual PCM", "focused position slider receives navigation keys", "native volume keys mute and restore actual audio", "Escape dismisses the volume popover first", "source selection stops the old stream without stealing browser focus", "video audio is decoded from its distinct track", "real video frames advance from red to green", "video Pause stops audio", "paused video seek changes the actual frame without playing", "Escape and default-focus Space close to the narrow browser", "replacing focused controls restores default preview keys", "Left/Right retain narrow browsing meaning inside Quick Look", "corrupt media reports a visible error", "another browser leaves source playback running and owner close stops it"], "media_error": error, "windows": windows()}
