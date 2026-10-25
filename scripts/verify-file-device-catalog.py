#!/usr/bin/env python3
"""Check headless GIO device observations against the private volume provider."""
import argparse
import importlib.util
import json
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


class Observations:
    def __init__(self, binary, session, name):
        self.generation = 0
        self.latest = None
        self.queue = queue.Queue(maxsize=64)
        self.log = (session.output / f"{name}.log").open("wb")
        self.raw = (session.output / f"{name}.jsonl").open("wb")
        self.process = subprocess.Popen([str(binary), "--internal-volume-monitor"],
                                        env=session.env, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=self.log,
                                        start_new_session=True)

        def read():
            try:
                while line := self.process.stdout.readline(4 * 1024 * 1024 + 1):
                    assert len(line) <= 4 * 1024 * 1024 and line.endswith(b"\n"), "invalid bounded frame"
                    self.raw.write(line)
                    self.raw.flush()
                    self.queue.put(json.loads(line))
                self.queue.put(RuntimeError("device helper output closed"))
            except Exception as error:
                self.queue.put(error)

        self.reader = threading.Thread(target=read, daemon=True)
        self.reader.start()

    def wait(self, predicate, *, events=False):
        deadline = time.monotonic() + 10
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("Timed out waiting for the device observation")
            try:
                frame = self.queue.get(timeout=remaining)
            except queue.Empty as error:
                raise TimeoutError("Timed out waiting for the device observation") from error
            if isinstance(frame, Exception):
                raise frame
            if "event" in frame:
                assert frame["version"] == 1 and frame["id"] > 0, frame
                if events and predicate(frame):
                    return frame
                continue
            assert frame["version"] == 2, "old helper has no device catalog"
            assert frame["generation"] > self.generation
            assert len(frame["mounts"]) <= 1024 and len(frame["devices"]) <= 1024
            self.generation = frame["generation"]
            self.latest = frame
            if not events and predicate(frame):
                return frame

    def send(self, ident, kind, **fields):
        frame = {"version": 1, "id": ident, "command": {"kind": kind, **fields}}
        self.process.stdin.write(json.dumps(frame).encode() + b"\n")
        self.process.stdin.flush()

    def close(self):
        self.process.stdin.close()
        try:
            assert self.process.wait(timeout=5) == 0, self.process.returncode
        finally:
            desktop.stop(self.process)
            self.reader.join(timeout=5)
            assert not self.reader.is_alive(), "helper output reader failed to retire"
            self.process.stdout.close()
            self.raw.close()
            self.log.close()


def verify(binary, session, commands=False, drives=False):
    modules = session.runtime / "gio-modules"
    modules.mkdir()
    flags = subprocess.check_output(["pkg-config", "--cflags", "--libs", "gio-2.0"], text=True).split()
    subprocess.run(["cc", "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror",
                    str(ROOT / "scripts/desktop-fixture/files-volume-monitor.c"),
                    "-o", str(modules / "libcanopa-volume-fixture.so"), *flags], check=True, timeout=30)
    media = session.runtime / "volumes"
    payload = media / "Fixture USB"
    payload.mkdir(parents=True)
    original = b"Actual private device contents.\n"
    (payload / "notes.txt").write_bytes(original)
    session.env.update(HOME=str(session.runtime / "home"), GIO_EXTRA_MODULES=str(modules),
                       GIO_USE_VOLUME_MONITOR="unix", GIO_USE_VFS="local",
                       CANOPA_FIXTURE_VOLUMES=str(media))
    session.start_private_bus()
    if drives:
        return verify_drives(binary, session, media, payload, original)
    if commands:
        return verify_commands(binary, session, media, payload, original)
    checks = []

    def devices(frame):
        return [device for device in frame["devices"] if device["name"] == "Fixture USB"]

    for mounted in (True, False):
        if not mounted:
            (media / "unmounted").touch()
        observations = Observations(binary, session, "mounted" if mounted else "unmounted")
        try:
            first = observations.wait(lambda frame: devices(frame))
            assert len(devices(first)) == 1, "volume and its mount were listed twice"
            device = devices(first)[0]
            assert device["instance"] != 0
            if mounted:
                root = next(mount for mount in first["mounts"] if mount["instance"] == device["mount"])
                assert root["path"] == str(payload), root
                assert device["removal"] == "unmount" and not device["can_mount"]
                assert (Path(root["path"]) / "notes.txt").read_bytes() == original
                checks.append("mounted local volume appears once with exact root and safe-removal capability")
            else:
                assert device["mount"] is None and device["can_mount"]
                assert device["removal"] is None
                checks.append("unmounted local volume is discovered without any mounted root")
            (media / "unplugged").touch()
            removed = observations.wait(lambda frame: not devices(frame))
            assert removed["generation"] > first["generation"]
            assert payload.is_dir(), "the old mount point deliberately remains"
            (media / "unplugged").unlink()
            returned = observations.wait(lambda frame: devices(frame))
            assert returned["generation"] > removed["generation"]
            assert len(devices(returned)) == 1
            checks.append(f"{'mounted' if mounted else 'unmounted'} hotplug publishes disappearance and a newer observation")
        finally:
            observations.close()
    events = (media / "events").read_text()
    assert "mount requested" not in events and "unmount requested" not in events, events
    assert (payload / "notes.txt").read_bytes() == original
    checks.append("observation never mounts, removes or changes files; both helpers exit on owner EOF")
    return {"status": "passed", "checks": checks,
            "scope": "headless private GIO catalog protocol only; not Iced controls, device commands or physical storage acceptance"}


def verify_commands(binary, session, media, payload, original):
    observations = Observations(binary, session, "commands")
    checks = []
    ident = 0

    def device():
        return next(d for d in observations.latest["devices"] if d["name"] == "Fixture USB")

    def start(kind, **fields):
        nonlocal ident
        ident += 1
        observations.send(ident, kind, instance=device()["instance"],
                          generation=observations.generation, **fields)
        return ident

    def event(ident, kind):
        return observations.wait(lambda f: f["id"] == ident and f["event"] == kind, events=True)

    def finished(ident, expected):
        result = event(ident, "finished")["outcome"]
        assert result["kind"] == expected, result
        assert (payload / "notes.txt").read_bytes() == original
        return result

    def remove():
        finished(start("remove", removal="unmount"), "removed")
        assert device()["mount"] is None and device()["can_mount"]

    def mounted():
        result = finished(ident, "mounted")
        assert result["root"] == str(payload) and device()["mount"] is not None

    try:
        first = observations.wait(lambda f: any(d["name"] == "Fixture USB" for d in f["devices"]))
        remove()
        checks.append("asynchronous unmount uses no force and publishes the unmounted catalog before completion")
        start("mount")
        mounted()
        checks.append("asynchronous mount returns the actual local root after its observation")
        observations.send(ident, "mount", instance=device()["instance"], generation=observations.generation)
        refusal = event(ident, "refused")
        assert refusal["terminal"] and "already been used" in refusal["message"]
        ident += 1
        observations.send(ident, "remove", instance=device()["instance"], generation=first["generation"], removal="unmount")
        refusal = event(ident, "refused")
        assert refusal["terminal"] and "observation has changed" in refusal["message"]
        assert device()["mount"] is not None
        checks.append("replayed request IDs and stale observations are refused before platform mutation")

        (media / "fail-unmount").touch()
        result = finished(start("remove", removal="unmount"), "failed")
        assert "busy" in result["message"] and device()["mount"] is not None
        (media / "fail-unmount").unlink()
        checks.append("platform errors retain actual mounted state and allow an explicit new request")

        (media / "hold-unmount").touch()
        before = (media / "events").read_text().count("unmount requested without force")
        active = start("remove", removal="unmount")
        session.wait_for(lambda: (media / "events").read_text().count("unmount requested without force") > before, "actual delayed unmount request")
        observations.send(active, "cancel")
        finished(active, "cancelled")
        assert device()["mount"] is not None
        (media / "hold-unmount").unlink()
        checks.append("cancellation waits for the delayed native callback and preserves the mounted device")

        remove()
        (media / "password-mount").touch()
        active = start("mount")
        question = event(active, "question")
        assert question["prompt"]["kind"] == "password" and question["question"] > 0
        answer = {"kind": "password", "username": "private-user", "domain": "private-domain",
                  "password": "private-secret", "anonymous": False}
        observations.send(active, "answer", question=question["question"] + 1, answer=answer)
        refusal = event(active, "refused")
        assert not refusal["terminal"] and "question has changed" in refusal["message"]
        assert device()["mount"] is None
        observations.send(active, "answer", question=question["question"], answer=answer)
        mounted()
        assert "credentials accepted without saving" in (media / "events").read_text()
        checks.append("real password interaction waits for the current answer and disables credential saving")

        remove()
        active = start("mount")
        event(active, "question")
        observations.send(active, "cancel")
        finished(active, "cancelled")
        assert device()["mount"] is None
        (media / "password-mount").unlink()
        checks.append("credential cancellation dismisses the outstanding question without mounting")

        (media / "question-mount").touch()
        active = start("mount")
        old = event(active, "question")
        current = event(active, "question")
        assert old["prompt"]["choices"][1] == "Old choice"
        assert current["prompt"]["choices"][1] == "Continue"
        assert current["question"] > old["question"]
        observations.send(active, "answer", question=old["question"], answer={"kind": "choice", "choice": 1})
        assert not event(active, "refused")["terminal"]
        assert device()["mount"] is None
        observations.send(active, "answer", question=current["question"], answer={"kind": "choice", "choice": 1})
        mounted()
        (media / "question-mount").unlink()
        checks.append("a newer native question invalidates the earlier choice while the same request remains pending")

        remove()
        (media / "hold-mount").touch()
        before = (media / "events").read_text().count("mount requested\n")
        active = start("mount")
        session.wait_for(lambda: (media / "events").read_text().count("mount requested\n") > before, "actual delayed mount before unplug")
        (media / "unplugged").touch()
        observations.wait(lambda f: not any(d["name"] == "Fixture USB" for d in f["devices"]))
        (media / "release-mount").touch()
        result = finished(active, "failed")
        assert "disconnected" in result["message"]
        (media / "unplugged").unlink()
        observations.wait(lambda f: any(d["name"] == "Fixture USB" for d in f["devices"]))
        assert device()["mount"] is None
        (media / "release-mount").unlink()
        checks.append("disconnect during a pending mount reports failure and never returns the retained mount-point directory as a mounted root")

        before = (media / "events").read_text().count("mount requested\n")
        start("mount")
        session.wait_for(lambda: (media / "events").read_text().count("mount requested\n") > before, "actual delayed mount before owner EOF")
    finally:
        observations.close()
    events = (media / "events").read_text()
    assert "mount cancelled" in events and "UNEXPECTED forced" not in events
    for path in [session.output / "commands.log", session.output / "commands.jsonl", media / "events"]:
        assert b"private-secret" not in path.read_bytes(), "credential escaped into helper output or fixture logs"
    assert (payload / "notes.txt").read_bytes() == original
    checks.append("owner EOF cancels and awaits the exact platform request before helper exit; private bytes remain unchanged")
    return {"status": "passed", "checks": checks,
            "scope": "private headless helper command/interaction protocol; parent admission, Iced controls, drive stop/eject and physical storage have separate acceptance"}


def verify_drives(binary, session, media, payload, original):
    checks = []
    for mode, removal in (("stop", "power-off"), ("eject", "eject")):
        marker = media / f"drive-{mode}"
        marker.touch()
        observations = Observations(binary, session, mode)
        ident = 0

        def present(frame):
            return next((d for d in frame["devices"] if d["name"] == "Fixture USB"), None)

        def start(capability=removal, generation=None):
            nonlocal ident
            ident += 1
            device = present(observations.latest)
            observations.send(ident, "remove", instance=device["instance"],
                              generation=observations.generation if generation is None else generation,
                              removal=capability)
            return ident

        def event(kind):
            return observations.wait(lambda f: f["id"] == ident and f["event"] == kind, events=True)

        def finished(expected):
            result = event("finished")["outcome"]
            assert result["kind"] == expected, result
            assert (payload / "notes.txt").read_bytes() == original
            return result

        def count(value):
            return (media / "events").read_text().count(value)

        def held():
            before = count(f"{mode} requested without force")
            start()
            session.wait_for(lambda: count(f"{mode} requested without force") > before,
                             f"actual held drive {mode}")

        try:
            first = observations.wait(present)
            assert present(first)["removal"] == removal, first
            assert present(first)["mount"] is not None
            # Stop wins even when the native drive can also eject and unmount.
            before = count("requested without force")
            start("unmount")
            refusal = event("refused")
            assert refusal["terminal"] and count("requested without force") == before, refusal
            checks.append(f"{mode}: catalog selects the preferred capability and refuses a different removal before GIO")

            (media / "fail-drive").touch()
            start()
            result = finished("failed")
            assert "busy" in result["message"] and present(observations.latest)["mount"] is not None
            (media / "fail-drive").unlink()
            checks.append(f"{mode}: actual native busy error retains mounted state and allows a new request")

            (media / "hold-drive").touch()
            held()
            before = count(f"{mode} cancelled")
            observations.send(ident, "cancel")
            finished("cancelled")
            assert count(f"{mode} cancelled") == before + 1
            assert present(observations.latest)["mount"] is not None
            checks.append(f"{mode}: cancellation settles through the actual delayed GDrive callback without removing the device")

            held()
            old_generation = observations.generation
            (media / "unplugged").touch()
            observations.wait(lambda f: present(f) is None)
            (media / "release-drive").touch()
            result = finished("failed")
            assert "disconnected" in result["message"] and present(observations.latest) is None
            (media / "unplugged").unlink()
            observations.wait(present)
            (media / "hold-drive").unlink()
            (media / "release-drive").unlink()
            before = count("requested without force")
            start(generation=old_generation)
            refusal = event("refused")
            assert refusal["terminal"] and "observation has changed" in refusal["message"]
            assert count("requested without force") == before
            checks.append(f"{mode}: unplug during removal fails and the earlier observation cannot target the reconnected device")

            start()
            finished("removed")
            assert present(observations.latest) is None
            assert not observations.latest["mounts"] and payload.is_dir()
            assert count(f"{mode} completed") == 1
            checks.append(f"{mode}: no-force native completion publishes disappearance before success and retains original private bytes")
        finally:
            observations.close()

        # A fresh helper sees the same unchanged private files as a new device.
        (media / "hold-drive").touch()
        observations = Observations(binary, session, f"{mode}-owner-eof")
        try:
            observations.wait(present)
            ident = 0
            held()
            before = count(f"{mode} cancelled")
        finally:
            observations.close()
        assert count(f"{mode} cancelled") == before + 1
        (media / "hold-drive").unlink()
        marker.unlink()
        checks.append(f"{mode}: owner EOF awaits cancellation and the exact native callback before helper exit")
    events = (media / "events").read_text()
    assert "UNEXPECTED forced" not in events and "unmount requested" not in events
    assert (payload / "notes.txt").read_bytes() == original
    (session.output / "device-events.log").write_text(events)
    return {"status": "passed", "checks": checks,
            "scope": "actual headless helper dispatch to private GDrive stop/eject callbacks; physical device behavior remains separate"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--commands", action="store_true")
    mode.add_argument("--drives", action="store_true")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    result = {"status": "interrupted"}
    with tempfile.TemporaryDirectory(prefix="canopa-files-native-device-catalog-") as name:
        runtime = Path(name)
        runtime.chmod(0o700)
        session = desktop.Session(runtime, output)
        try:
            result = verify(args.binary.resolve(), session, args.commands, args.drives)
        except Exception as error:
            result = {"status": "failed", "error": str(error)}
            raise
        finally:
            (output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
            session.close()
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
