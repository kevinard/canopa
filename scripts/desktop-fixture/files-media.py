"""Private media fixtures and isolated audio services for the Iced media cases.

The former GTK playback walkthrough was removed with the GTK frontend; the
Iced controls, decoded pixels and recorded PCM are verified by files-iced-media.py.
"""
import math
import struct
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


def setup(session, fixture, formats_only=False):
    audio = session.runtime / "audio"
    audio.mkdir()
    # Same isolated protocol setup as the shell fixture, with a dummy clock and
    # policy-only WirePlumber for actual streams. No hardware monitor is loaded.
    session.env.update(PIPEWIRE_RUNTIME_DIR=str(audio), PIPEWIRE_CONFIG_DIR=str(audio),
                       PIPEWIRE_CONFIG_PREFIX="", PIPEWIRE_REMOTE="pipewire-0",
                       PULSE_RUNTIME_PATH=str(audio / "pulse"), PULSE_SERVER=f"unix:{audio}/pulse/native",
                       ALSA_CONFIG_PATH=str(audio / "alsa.conf"), SDL_AUDIODRIVER="pulseaudio",
                       LANG="C.UTF-8")
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
