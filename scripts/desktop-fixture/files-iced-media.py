"""Real Iced media controls, decoded pixels and private monitored PCM."""
import array
import importlib.util
import json
import subprocess
from pathlib import Path
import time

spec = importlib.util.spec_from_file_location("files_media_input", Path(__file__).with_name("files-media.py"))
media_input = importlib.util.module_from_spec(spec)
spec.loader.exec_module(media_input)


def setup(session, fixture, formats_only=False):
    # The shared fixture starts only private PipeWire/Pulse policy and a null
    # output. Iced observations use real input, focus borders, pixels and PCM.
    media_input.setup(session, fixture, formats_only=formats_only)
    # Open must target only our private argv recorder, including corrupt media
    # whose extension still determines the normal application MIME association.
    application = session.runtime / "data/applications/canopa-files-fixture.desktop"
    application.write_text(application.read_text().replace(
        "MimeType=text/plain;", "MimeType=text/plain;video/webm;"))
    associations = session.runtime / "config/mimeapps.list"
    associations.write_text(associations.read_text() + "video/webm=canopa-files-fixture.desktop;\n")
    association = session.run(["gio", "mime", "video/webm"])
    assert "canopa-files-fixture.desktop" in association.splitlines()[0], association


def verify(session, fixture, browser, launched, scheme, scale, process, formats_only=False):
    current_browser = browser
    recording = session.output / "captured.pcm"
    measurements = []
    checks = []
    focus_color = (176, 222, 195) if scheme == "dark" else (37, 105, 72)

    def windows():
        return json.loads(session.run(["niri", "msg", "--json", "windows"]))

    def title(value):
        return next((item for item in windows() if item["title"] == value), None)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "native media window focus")
        time.sleep(.15)

    def keys(*names):
        for name in names:
            session.run(["wtype", "-k", name])
            time.sleep(.15)

    def burst(*names):
        session.run(["wtype", *[part for name in names for part in ("-k", name)]])
        time.sleep(.15)

    def chord(key, modifier="ctrl"):
        session.run(["wtype", "-M", modifier, "-k", key, "-m", modifier])
        time.sleep(.15)

    def absent(window):
        session.wait_for(lambda: all(w["id"] != window["id"] for w in windows()), "native media window retirement")

    def navigate(path):
        focus(current_browser)
        chord("l")
        # Long virtual-keyboard text takes per-key compositor roundtrips; media
        # rendering in this nested software desktop can exceed the10s command
        # limit. Paste the path through the actual text-input clipboard instead.
        # Keyboard media/source navigation is exercised separately below.
        # Its forked clipboard owner must not keep check_output pipes open.
        with (session.output / "clipboard.log").open("ab") as log:
            subprocess.run(["wl-copy", "--", str(path)], env=session.env,
                           stdout=log, stderr=log, check=True, timeout=10)
        chord("v")
        keys("Return")
        session.wait_for(lambda: title(f"{path.parent.name} — Canopa Files"), "media source folder")

    def preview(path, opening=True):
        navigate(path)
        if opening:
            keys("space")
        result = session.wait_for(lambda: title(f"Quick Look: {path.name} — Canopa Files"), "source-bound media preview")
        assert result["is_floating"], result
        focus(result)
        return result

    def bounds(window):
        actual = next(w for w in windows() if w["id"] == window["id"])
        x, y = actual["layout"]["tile_pos_in_workspace_view"]
        width, height = actual["layout"]["window_size"]
        return x, y, width, height

    def focus_play(window):
        # Observe the visible focus outline on the first transport button. The
        # source may still be preparing; Tab reaches Play only once it enables.
        # Header Open and the seek/volume rows lie outside this action rectangle.
        def outlined():
            x, y, _, height = bounds(window)
            frame = session.capture("media-control-focus.png")
            crop = frame.crop(tuple(round(v * scale) for v in (x + 6, y + height - 70, x + 70, y + height - 38)))
            for row in range(crop.height):
                run = 0
                for column in range(crop.width):
                    pixel = crop.getpixel((column, row))
                    run = run + 1 if all(abs(a - b) <= 6 for a, b in zip(pixel, focus_color)) else 0
                    if run >= round(28 * scale):
                        return True
            return False
        deadline = time.monotonic() + 12
        while not outlined():
            assert time.monotonic() < deadline, "Tab did not expose the enabled Play focus outline"
            keys("Tab")

    def samples(seconds=.15):
        start = recording.stat().st_size // 2 * 2
        size = round(seconds * 48000) * 2
        session.wait_for(lambda: recording.stat().st_size >= start + size, "fresh private monitor PCM")
        with recording.open("rb") as stream:
            stream.seek(start)
            return array.array("h", stream.read(size))

    def signal(expected=None, minimum=1000, maximum=8000):
        values = samples()
        peak = max(map(abs, values), default=0)
        frequency = sum(a <= 0 < b for a, b in zip(values, values[1:])) * 48000 / len(values)
        measurements.append({"expected_hz": expected, "measured_hz": frequency, "peak": peak})
        (session.output / "media-pcm.json").write_text(json.dumps(measurements, indent=2))
        if expected is None:
            return peak <= 20
        return minimum <= peak <= maximum and abs(frequency - expected) < 25

    def heard(frequency, minimum=1000, maximum=8000):
        session.wait_for(lambda: signal(frequency, minimum, maximum), f"actual decoded {frequency}Hz PCM")

    def silent():
        session.wait_for(signal, "actual paused/muted/stopped PCM silence")

    def video(window, color, name):
        def rendered():
            x, y, width, height = bounds(window)
            frame = session.capture(name)
            area = frame.crop(tuple(round(v * scale) for v in
                                    (x + 25, y + 70, x + width - 25, y + min(height - 150, height * .65))))
            return sum(all(abs(a - b) <= 20 for a, b in zip(pixel, color)) for pixel in area.get_flattened_data()) >= 1000
        session.wait_for(rendered, "actual decoded video pixels")

    session.wait_for(lambda: recording.exists() and recording.stat().st_size > 24000, "private audio calibration PCM")
    assert max(map(abs, array.array("h", recording.read_bytes())), default=0) > 1000, "private audio calibration was silent"
    checks.append("isolated null-output monitor records actual calibration PCM")
    focus(current_browser)
    session.run(["niri", "msg", "action", "set-window-width", "360"])

    if formats_only:
        for name, _, frequency in media_input.AUDIO_FORMATS:
            path = fixture / "Media formats" / name
            floating = preview(path)
            if name == "A-mp3.mp3":
                for _ in range(8):
                    focus(floating)
                    focus_play(floating)
                    silent()
                    keys("Escape")
                    absent(floating)
                    keys("space")
                    floating = session.wait_for(lambda: title(f"Quick Look: {name} — Canopa Files"), "reopened tagged MP3")
                focus(floating)
                focus_play(floating)
                checks.append("tagged MP3 repeatedly prepares enabled transport before each close without a decoder abort")
            silent()
            focus_play(floating)
            keys("space")
            heard(frequency)
            keys("Tab")  # Seek.
            burst("Right", "Right", "Right")
            heard(frequency + 220)
            keys("Home")
            heard(frequency)
            chord("Tab", "shift")  # Play/Pause.
            keys("space")
            silent()
            keys("Escape")
            absent(floating)
            checks.append(f"{name}: no autoplay, actual first/seek/rewind tone and pause")
        floating = preview(fixture / "Media formats/E-video.mp4")
        video(floating, (253, 0, 0), "media-h264-red.png")
        silent()
        focus_play(floating)
        keys("Tab", "Right")
        video(floating, (0, 128, 0), "media-h264-green.png")
        keys("Right")
        video(floating, (0, 0, 253), "media-h264-blue.png")
        keys("Home")
        video(floating, (253, 0, 0), "media-h264-rewind.png")
        chord("Tab", "shift")
        keys("space")
        heard(990)
        video(floating, (0, 128, 0), "media-h264-playing-green.png")
        video(floating, (0, 0, 253), "media-h264-playing-blue.png")
        keys("space")
        silent()
        keys("Escape")
        absent(floating)
        checks.append("H.264/AAC: paused seeks and rewind, advancing green/blue pixels during playback, decoded audio and pause")
    else:
        audio = fixture / "Media/A-audio.wav"
        floating = preview(audio)
        assert abs(bounds(floating)[2] - 560) <= 4, floating
        actual_browser = next(w for w in windows() if w["id"] == current_browser["id"])
        assert abs(actual_browser["layout"]["window_size"][0] - 360) <= 4, actual_browser
        silent()
        focus_play(floating)
        keys("space")
        heard(440)
        keys("Tab", "Tab", "space")  # Seek, Mute.
        silent()
        keys("space")
        heard(440)
        keys("Tab", "Home")  # Volume zero.
        silent()
        burst(*(["Right"] * 10))
        heard(440, 2500, 3500)
        keys("End")
        heard(440, 5500, 6500)
        session.capture("media-audio-controls.png")
        checks.append("WAV starts paused, floats independently, and actual PCM follows play/mute/volume including rapid keys")
        keys("Escape")
        absent(floating)
        silent()
        checks.append("closing a playing preview stops its actual audio")

        path = fixture / "Media/B-video.webm"
        floating = preview(path)
        video(floating, (253, 0, 0), "media-vp8-red.png")
        silent()
        outputs = json.loads(session.run(["niri", "msg", "--json", "outputs"]))
        logical = next(item["logical"] for item in outputs.values() if item.get("logical"))
        pointer = session.build_pointer()
        x, y, _, height = bounds(floating)
        session.run([str(pointer), str(round((x + 200) / logical["width"] * 1600)),
                     str(round((y + height - 90) / logical["height"] * 1000)), "move"])
        keys("Down")
        session.wait_for(lambda: title("Quick Look: C-text.txt — Canopa Files"), "unfocused hovered seek does not capture source Down")
        keys("Up")
        session.wait_for(lambda: title("Quick Look: B-video.webm — Canopa Files"), "source Up restores video selection")
        video(floating, (253, 0, 0), "media-hover-source-return.png")
        checks.append("hovering an unfocused seek slider preserves the source navigation keys")
        focus_play(floating)
        keys("Tab")
        burst("Right", "Right")
        video(floating, (0, 128, 0), "media-vp8-green.png")
        silent()
        keys("Home")
        video(floating, (253, 0, 0), "media-vp8-rewind.png")
        session.run(["niri", "msg", "action", "set-window-width", "340"])
        session.run(["niri", "msg", "action", "set-window-height", "360"])
        keys("Right", "Right")
        video(floating, (0, 128, 0), "media-vp8-narrow.png")
        keys("Left")
        video(floating, (253, 0, 0), "media-vp8-narrow-left.png")
        assert title("Media — Canopa Files"), "focused seek Left navigated the source to its parent"
        keys("Home")
        chord("Tab", "shift")
        keys("space")
        heard(660)
        video(floating, (0, 128, 0), "media-vp8-playing-green.png")
        keys("space", "Tab", "Home")
        video(floating, (253, 0, 0), "media-vp8-playing-rewind.png")
        chord("Tab", "shift")
        keys("space")
        heard(660)
        checks.append("VP8/Vorbis advancing playback pixels, paused pixels, composed seek, rewind and focused controls work in a narrow floating window")

        # A second Browser has independent selection and cannot take the source.
        before = {w["id"] for w in windows()}
        focus(current_browser)
        chord("n")
        other = session.wait_for(lambda: next((w for w in windows() if w["id"] not in before and not w["is_floating"]), None), "second media Browser")
        owner = current_browser
        current_browser = other
        focus(other)
        session.run(["niri", "msg", "action", "set-window-width", "360"])
        other_preview = preview(fixture / "Media/C-text.txt")
        assert other_preview["id"] != floating["id"], "other Browser reused its owner's Quick Look"
        assert title("Quick Look: B-video.webm — Canopa Files")["id"] == floating["id"]
        heard(660)
        session.capture("media-independent-browsers.png")
        keys("Escape")
        absent(other_preview)
        focus(owner)
        chord("w")
        absent(owner)
        absent(floating)
        silent()
        checks.append("other Browser selection preserves playback; closing the source owner retires its preview and audio")

        floating = preview(path)
        video(floating, (253, 0, 0), "media-source-red.png")
        focus_play(floating)
        keys("space")
        heard(660)
        preview(fixture / "Media/C-text.txt", opening=False)
        silent()
        session.capture("media-source-text.png")
        checks.append("source selection follows to text and stops the previous decoder")
        broken = fixture / "Media/D-broken.webm"
        floating = preview(broken, opening=False)
        session.wait_for(lambda: "[canopa-files media]" in (session.output / "files.log").read_text(), "actual corrupt-media decoder failure")
        time.sleep(.2)
        session.capture("media-error.png")
        launched.unlink(missing_ok=True)
        keys("Tab")
        session.capture("media-error-focused.png")
        keys("space")  # Failed media retains Open.
        session.wait_for(lambda: launched.exists() and json.loads(launched.read_text()) == [str(broken)], "normal application fallback for corrupt media")
        keys("Escape")
        absent(floating)
        checks.append("corrupt decoder failure retains actual Open; media-error.png requires separate rendered explanation review")

        floating = preview(audio)
        focus_play(floating)
        keys("space")
        heard(440)

    focus(current_browser)
    chord("w")
    # Session.wait_for treats any service exit as unexpected. Observe this one
    # expected exit first, and retain cleanup ownership if the deadline fails.
    assert process.wait(timeout=15) == 0, process.returncode
    session.processes.remove(process)
    absent(current_browser)
    if not formats_only:
        absent(floating)
    silent()
    checks.append("last Browser closure drains decoder cleanup before the owning process exits successfully")
    return {"status": "passed", "scheme": scheme, "scale": scale, "checks": checks,
            "scope": "native Iced media controls, decoded pixels and private PCM; accessibility/performance and full product acceptance remain separate"}
