"""Hardware-backed, private ScreenCast consumer acceptance helpers."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import shlex
import subprocess
import time

from native_compositor import SNAPSHOT_STORE, stage_niri

_spec = importlib.util.spec_from_file_location("desktop", Path(__file__).with_name("validate-desktop.py"))
desktop = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(desktop)

FRONTEND = "org.freedesktop.portal.Desktop"
BACKEND = "org.freedesktop.impl.portal.desktop.canopa"
PATH = "/org/freedesktop/portal/desktop"
PUBLIC = "org.freedesktop.portal.ScreenCast"
COLORS = [(180, 20, 40), (10, 40, 140), (20, 150, 40), (180, 154, 24)]


def stream_mapping(properties):
    """Validate optional public EI pairing metadata before consuming its video."""
    identifier = properties.get("mapping_id")
    if identifier is None:
        return None
    assert isinstance(identifier, str) and identifier and len(identifier.encode()) <= 256
    assert all(character.isprintable() for character in identifier)
    size = properties["size"]
    assert len(size) == 2 and all(isinstance(value, int) and 1 <= value <= 32768 for value in size)
    return {"id": identifier, "size": list(size)}


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


class MediaSession(desktop.Session):
    def __init__(self, runtime, output, compositor, checksum, *, reference_frozen=False):
        super().__init__(runtime, output)
        if reference_frozen:
            canonical = SNAPSHOT_STORE / checksum / "niri"
            if compositor.resolve() != canonical.resolve() or canonical.is_symlink():
                raise ValueError("Only a content-addressed frozen compositor can be referenced")
        staged = stage_niri({"source": str(compositor), "sha256": checksum}, output)
        self.compositor = Path(staged["snapshot"])
        self.checksum = checksum
        self.env["PATH"] = str(self.compositor.parent) + os.pathsep + self.env["PATH"]
        self.env.pop("LIBGL_ALWAYS_SOFTWARE", None)
        self.env.update(PIPEWIRE_RUNTIME_DIR=str(runtime), PIPEWIRE_REMOTE="pipewire-0")
        self.frames = []
        self.cursor_frames = []

    @staticmethod
    def remote(connection, key):
        from gi.repository import Gio, GLib
        reply, descriptors = connection.call_with_unix_fd_list_sync(
            FRONTEND, PATH, PUBLIC, "OpenPipeWireRemote", GLib.Variant("(oa{sv})", (key, {})),
            GLib.VariantType.new("(h)"), Gio.DBusCallFlags.NONE, 10000, None, None)
        return descriptors.get(reply.unpack()[0])

    def start_niri(self, config, *, width=1280, height=800, scale=1.0, service_directories=()):
        self.pixel_size = (width, height)
        self.scale = scale
        self.start_private_bus(service_directories=service_directories)
        self.start(["pipewire"], "pipewire.log")
        self.wait_for(lambda: (self.runtime / "pipewire-0").is_socket(), "private PipeWire")
        # Policy only: link the fixture's streams without discovering hardware.
        self.start(["wireplumber", "--profile", "policy"], "wireplumber.log")
        self.wait_for(lambda: any(v.get("info", {}).get("props", {}).get("application.name", "").startswith("WirePlumber")
                                 for v in self.registry()), "private WirePlumber policy")
        self.start(["weston", "--backend=headless", "--renderer=gl", "--fake-seat",
                    f"--width={width}", f"--height={height}", "--socket=wayland-parent",
                    "--no-config", "--idle-time=0"], "weston.log")
        self.wait_for(lambda: (self.runtime / "wayland-parent").is_socket(), "hardware GL Weston")
        self.env["WAYLAND_DISPLAY"] = "wayland-parent"
        path = self.runtime / "niri.kdl"
        path.write_text(config)
        self.start([str(self.compositor), "-c", str(path)], "niri.log")
        self.env["NIRI_SOCKET"] = str(self.wait_for(lambda: list(self.runtime.glob("niri.*.sock")), "Niri IPC")[0])
        self.env["WAYLAND_DISPLAY"] = str(self.wait_for(
            lambda: [p for p in self.runtime.glob("wayland-*") if p.is_socket() and p.name != "wayland-parent"],
            "private Niri display")[0])
        def outputs():
            try:
                values = json.loads(self.run(["niri", "msg", "--json", "outputs"]))
                return values if any(v.get("logical") for v in values.values()) else None
            except (subprocess.SubprocessError, json.JSONDecodeError):
                return None
        values = self.wait_for(outputs, "Niri output")
        name = next(name for name, value in values.items() if value.get("logical"))
        self.run(["niri", "msg", "output", name, "scale", str(scale)])
        self.wait_for(lambda: abs(outputs()[name]["logical"]["scale"] - scale) < .001, "output scale")
        return name

    def registry(self):
        return json.loads(self.run(["pw-dump"]))

    @staticmethod
    def pattern(image):
        from PIL import Image, ImageChops
        rgb = image.convert("RGB")
        boxes = []
        for color in COLORS:
            channels = ImageChops.difference(rgb, Image.new("RGB", rgb.size, color)).split()
            delta = ImageChops.lighter(ImageChops.lighter(channels[0], channels[1]), channels[2])
            box = delta.point(lambda value: 255 if value <= 2 else 0).getbbox()
            if box is None:
                return None
            boxes.append(box)
        x, y = min(b[0] for b in boxes), min(b[1] for b in boxes)
        right, bottom = max(b[2] for b in boxes), max(b[3] for b in boxes)
        sx, sy = (right - x) / 400, (bottom - y) / 300
        expected = [(0, 0, 137, 93), (0, 93, 137, 300), (137, 0, 400, 181), (137, 181, 400, 300)]
        if abs(sx - sy) > .03 or sx < .5:
            return None
        for actual, source in zip(boxes, expected):
            target = (x + source[0] * sx, y + source[1] * sy, x + source[2] * sx, y + source[3] * sy)
            if any(abs(a - b) > 3 for a, b in zip(actual, target)):
                return None
        return (x, y, right, bottom)

    def check_stream(self, connection, key, results, kind, logical, expected_box, close, *, label=None, expect_pattern=True):
        import gi
        gi.require_version("Gst", "1.0")
        gi.require_version("GstVideo", "1.0")
        from gi.repository import Gio, GLib, Gst, GstVideo
        from PIL import Image
        streams = results["streams"]
        assert len(streams) == 1 and results.get("persist_mode", 0) == 0, results
        node, properties = streams[0]
        serial = properties["pipewire-serial"]
        assert node > 0 and serial > 0 and properties["source_type"] == kind, streams
        if kind == 1:
            assert tuple(properties["size"]) == (logical["width"], logical["height"])
            assert tuple(properties["position"]) == (logical["x"], logical["y"])
        else:
            assert "position" not in properties, properties
            if "mapping_id" not in properties:
                assert "size" not in properties, properties
        mapping = stream_mapping(properties)
        def producer_alive():
            return any(v["id"] == node and int(v.get("info", {}).get("props", {}).get("object.serial", 0)) == serial
                       for v in self.registry())
        assert producer_alive(), (node, serial)
        foreign = Gio.DBusConnection.new_for_address_sync(self.env["DBUS_SESSION_BUS_ADDRESS"],
            Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
        try:
            denied = False
            try:
                unwanted = self.remote(foreign, key)
                os.close(unwanted)
            except GLib.Error as error:
                denied = Gio.DBusError.get_remote_error(error) == "org.freedesktop.DBus.Error.AccessDenied"
            assert denied, "foreign caller obtained a PipeWire remote"
        finally:
            foreign.close_sync(None)
        fd = self.remote(connection, key)
        pipeline = None
        try:
            # FD is mandatory. The plugin duplicates it before pw_context_connect_fd.
            for name in ("XDG_RUNTIME_DIR", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME",
                         "PIPEWIRE_RUNTIME_DIR", "PIPEWIRE_REMOTE", "WAYLAND_DISPLAY"):
                os.environ[name] = self.env[name]
            os.environ.pop("DISPLAY", None)
            os.environ.pop("LIBGL_ALWAYS_SOFTWARE", None)
            Gst.init(None)
            def decode(sample):
                video = GstVideo.VideoInfo.new_from_caps(sample.get_caps())
                assert 0 < video.width <= 32768 and 0 < video.height <= 32768
                assert 0 < video.stride[0] * video.height <= 64 * 1024 * 1024
                buffer = sample.get_buffer()
                ok, mapping = buffer.map(Gst.MapFlags.READ)
                assert ok
                try:
                    image = Image.frombytes("RGBA", (video.width, video.height),
                        bytes(mapping.data)[video.offset[0]:], "raw", "RGBA", video.stride[0])
                finally:
                    buffer.unmap(mapping)
                return image, buffer.offset
            pipeline = Gst.parse_launch(f"pipewiresrc fd={fd} target-object={serial} ! videoconvert ! video/x-raw,format=RGBA ! appsink name=frames sync=false max-buffers=2 drop=true")
            sink = pipeline.get_by_name("frames")
            assert pipeline.set_state(Gst.State.PLAYING) != Gst.StateChangeReturn.FAILURE
            end = time.monotonic() + 20
            frame = box = None
            while time.monotonic() < end:
                sample = sink.emit("try-pull-sample", Gst.SECOND)
                message = pipeline.get_bus().pop_filtered(Gst.MessageType.ERROR)
                if message:
                    raise RuntimeError(str(message.parse_error()))
                if sample is None:
                    continue
                frame, sequence = decode(sample)
                box = self.pattern(frame)
                if box is not None or not expect_pattern:
                    break
            if frame is not None:
                frame.save(self.output / f"media-{kind}{'-'+label if label else ''}.png")
            assert frame is not None, "no video frame"
            if mapping is not None:
                assert all(abs(pixels - logical * self.scale) <= 3 for pixels, logical in zip(frame.size, mapping["size"])), (frame.size, mapping, self.scale)
            assert box is not None or not expect_pattern, "no frame containing the asymmetric source pattern"
            resume_sequence = None
            if kind == 1:
                assert frame.size == self.pixel_size, frame.size
                if expect_pattern:
                    assert all(abs(a - b) <= 3 for a, b in zip(box, expected_box)), (box, expected_box)
            else:
                assert all(abs(a - b) <= 3 for a, b in zip(box, (0, 0, *frame.size))), (box, frame.size)
                assert sequence != Gst.BUFFER_OFFSET_NONE
                assert pipeline.set_state(Gst.State.PAUSED) != Gst.StateChangeReturn.FAILURE
                _, state, _ = pipeline.get_state(5 * Gst.SECOND)
                assert state == Gst.State.PAUSED
                def paused():
                    return any(v["id"] == node and int(v.get("info", {}).get("props", {}).get("object.serial", 0)) == serial
                               and v.get("info", {}).get("state") in ("idle", "suspended") for v in self.registry())
                self.wait_for(paused, "producer paused without source damage", timeout=5)
                assert pipeline.set_state(Gst.State.PLAYING) != Gst.StateChangeReturn.FAILURE
                end = time.monotonic() + 5
                resumed = False
                while time.monotonic() < end:
                    fresh = sink.emit("try-pull-sample", Gst.SECOND)
                    if fresh is not None:
                        image, fresh_sequence = decode(fresh)
                        if fresh_sequence != Gst.BUFFER_OFFSET_NONE and fresh_sequence > sequence:
                            assert image.size == frame.size and self.pattern(image) == box
                            image.save(self.output / "media-2-resumed.png")
                            resume_sequence = fresh_sequence
                            resumed = True
                            break
                assert resumed, "static window produced no fresh frame after consumer pause/resume"
            close()
            self.wait_for(lambda: not producer_alive(), "selected producer removed after close", timeout=15)
            self.frames.append({"source_type": kind, "node": node, "pipewire_serial": serial,
                                "case": label, "mapping": mapping,
                                "dimensions": list(frame.size), "pattern_bounds": list(box) if box else None,
                                "consumer": "OpenPipeWireRemote fd and target-object serial", "producer_retired": True,
                                "first_frame_sequence": sequence, "resumed_frame_sequence": resume_sequence,
                                "static_window_resume_verified": kind == 2})
        finally:
            if pipeline is not None:
                pipeline.set_state(Gst.State.NULL)
            os.close(fd)

    def check_cursor_stream(self, connection, key, results, kind, mode, expected_box, pointer, close):
        from PIL import Image, ImageChops
        consumer = self.output / "cursor-consumer"
        if not consumer.exists():
            flags = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "--libs", "libpipewire-0.3"], text=True))
            subprocess.run(["cc", "-Wall", "-Wextra", "-Werror",
                str(Path(__file__).parent / "desktop-fixture/screencast-consumer.c"), *flags,
                "-o", str(consumer)], check=True, timeout=30)
        node, properties = results["streams"][0]
        serial = properties["pipewire-serial"]
        assert len(results["streams"]) == 1 and properties["source_type"] == kind
        mapping = stream_mapping(properties)
        def producer_alive():
            return any(v["id"] == node and int(v.get("info", {}).get("props", {}).get("object.serial", 0)) == serial
                       for v in self.registry())
        assert producer_alive()
        directory = self.output / f"cursor-{kind}-{mode}"
        directory.mkdir()
        fd = self.remote(connection, key)
        try:
            with (directory / "consumer.log").open("w") as log:
                completed = subprocess.run([str(consumer), str(fd), str(serial), str(directory), str(mode)],
                    pass_fds=(fd,), env=self.env, text=True, stdout=subprocess.PIPE, stderr=log, timeout=25)
            (directory / "raw-result.json").write_text(completed.stdout)
            assert completed.returncode == 0, (completed.returncode, (directory / "consumer.log").read_text())
            report = json.loads(completed.stdout)
        finally:
            os.close(fd)
        size = (report["width"], report["height"])
        raw = (directory / "frame.rgba").read_bytes()
        assert len(raw) == size[0] * size[1] * 4
        frame = Image.frombytes("RGBA", size, raw)
        frame.save(directory / "frame.png")
        box = self.pattern(frame)
        assert box and all(abs(a-b) <= 3 for a, b in zip(box, expected_box)), (box, expected_box)
        if kind == 1:
            assert size == self.pixel_size
        else:
            assert all(abs(a-b) <= 3 for a, b in zip(box, (0, 0, *size)))
        def marker_box(image):
            rgb = image.convert("RGB")
            mask = Image.new("L", image.size)
            counts = []
            for color in [(255, 0, 255), (0, 255, 255)]:
                channels = ImageChops.difference(rgb, Image.new("RGB", rgb.size, color)).split()
                delta = ImageChops.lighter(ImageChops.lighter(channels[0], channels[1]), channels[2])
                found = delta.point(lambda value: 255 if value <= 4 else 0)
                counts.append(found.histogram()[255])
                mask = ImageChops.lighter(mask, found)
            return mask.getbbox(), counts
        embedded, counts = marker_box(frame)
        cursor_box = None
        if mode == 2:
            assert not report["cursor_valid"] and embedded and min(counts) > 20, (report, embedded, counts)
            expected = (pointer[0] - round(3*self.scale), pointer[1] - round(5*self.scale))
            assert all(abs(a-b) <= 3 for a, b in zip(embedded[:2], expected)), (embedded, expected)
            assert all(abs(v - 24*self.scale) <= 3 for v in (embedded[2]-embedded[0], embedded[3]-embedded[1]))
        else:
            assert embedded is None, (mode, embedded)
            if mode == 1:
                assert not report["cursor_valid"]
            else:
                assert report["cursor_valid"]
                assert all(abs(a-b) <= 2 for a, b in zip(report["position"], pointer)), (report, pointer)
                assert all(abs(a-b) <= 2 for a, b in zip(report["hotspot"], (3*self.scale, 5*self.scale)))
                cursor_size = tuple(report["cursor_size"])
                assert all(abs(value - 24*self.scale) <= 2 for value in cursor_size), cursor_size
                cursor_raw = (directory / "cursor.rgba").read_bytes()
                assert len(cursor_raw) == cursor_size[0] * cursor_size[1] * 4
                cursor = Image.frombytes("RGBA", cursor_size, cursor_raw)
                cursor.save(directory / "cursor.png")
                cursor_box, counts = marker_box(cursor)
                assert cursor_box and min(counts) > 20, (cursor_box, counts)
        close()
        self.wait_for(lambda: not producer_alive(), "cursor producer retired", timeout=15)
        self.cursor_frames.append({"source_type":kind, "cursor_mode":mode, "node":node, "mapping":mapping,
            "pipewire_serial":serial, **report, "pattern_bounds":list(box),
            "embedded_bounds":list(embedded) if embedded else None,
            "cursor_marker_bounds":list(cursor_box) if cursor_box else None,
            "producer_retired":True, "consumer":"restricted frontend FD and SPA cursor bitmap"})

    def media_result(self):
        assert digest(self.compositor) == self.checksum
        return {"niri_sha256": self.checksum, "frames": self.frames, "cursor_frames":self.cursor_frames}
