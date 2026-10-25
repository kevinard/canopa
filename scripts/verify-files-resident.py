#!/usr/bin/env python3
"""Exercise Files residency and activation in an owned private session."""
import argparse
import hashlib
import importlib.util
import json
import os
import resource
import signal
from pathlib import Path
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def metrics(pid):
    """Three seconds of the owned process tree; summed RSS counts shared pages."""
    def sample():
        pending, result = [pid], {}
        while pending:
            current = pending.pop()
            if current in result:
                continue
            result[current] = desktop.sample_process(current)
            for task in Path(f"/proc/{current}/task").iterdir():
                try:
                    pending.extend(int(child) for child in (task / "children").read_text().split())
                except FileNotFoundError:
                    pass
        return result
    before = sample()
    time.sleep(3)
    after = sample()
    assert before.keys() == after.keys(), "resident process tree changed during idle measurement"
    for child in before:
        assert before[child]["start_ticks"] == after[child]["start_ticks"]
    elapsed = after[pid]["monotonic_seconds"] - before[pid]["monotonic_seconds"]
    ticks = sum(after[p]["cpu_ticks"] - before[p]["cpu_ticks"] for p in before)
    return {"elapsed_seconds": elapsed, "cpu_percent_one_core": 100 * ticks / os.sysconf("SC_CLK_TCK") / elapsed,
            "summed_rss_kib": sum(s["rss_kib"] for s in after.values()), "processes": len(after),
            "scope": "Files owner and its descendants; summed RSS includes shared pages; private debug build"}


def volume_fixture(session):
    modules = session.output / "gio-modules"
    modules.mkdir()
    flags = subprocess.check_output(["pkg-config", "--cflags", "--libs", "gio-2.0"], text=True).split()
    subprocess.run(["cc", "-shared", "-fPIC", "-Wall", "-Wextra", "-Werror",
                    str(ROOT / "scripts/desktop-fixture/files-volume-monitor.c"),
                    "-o", str(modules / "libcanopa-volume-fixture.so"), *flags], check=True, timeout=30)
    media = session.runtime / "volumes"
    (media / "Fixture USB").mkdir(parents=True)
    session.env.update(GIO_EXTRA_MODULES=str(modules), GIO_USE_VOLUME_MONITOR="unix",
                       GIO_USE_VFS="local", CANOPA_FIXTURE_VOLUMES=str(media))


def verify(binary, session, headless):
    home = session.runtime / "home"
    root = home / "Resident café"
    root.mkdir(parents=True)
    session.env["HOME"] = str(home)
    volume_fixture(session)
    if headless:
        session.start_private_bus()
    else:
        session.start_niri('animations { off; }\nxwayland-satellite { off; }\n'
                           'hotkey-overlay { skip-at-startup; }\nlayout { gaps 0; }\n')
        session.start(["wtype", "-s", "600000"], "keyboard.log")
    events = session.output / "notifications.json"
    session.start([sys.executable, str(ROOT / "scripts/desktop-fixture/files-notifications.py"), str(events)], "notifications.log")
    session.wait_for(events.with_suffix(".ready").exists, "private notification receiver")
    owner = session.start([str(binary), "--defaults", "--daemon"], "files.log")

    def live(condition):
        if owner.poll() is not None:
            raise RuntimeError(f"Files owner exited {owner.returncode}: {(session.output / 'files.log').read_text()}")
        return condition()

    session.wait_for(lambda: live(lambda: (session.runtime / "state/canopa-files/operations.sqlite3").is_file()), "resident backend startup")
    checks = ["resident startup opens the operation history before any browser"]

    def command(*args, ok=True, cwd=None):
        process = subprocess.run([str(binary), *map(str, args)], env=session.env, cwd=cwd,
                                 text=True, capture_output=True, timeout=10)
        assert (process.returncode == 0) == ok, process.stderr
        return process

    command("--daemon")
    result = {"pid": owner.pid, "idle_before_gui": metrics(owner.pid)}
    if headless:
        assert not any(name in session.env for name in ("DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET"))
        refusal = command(root, ok=False)
        assert "without a display" in refusal.stderr, refusal.stderr
        assert owner.poll() is None
        # Stop only this owner's exact private helper. Orderly shutdown must
        # retain ownership until the helper can observe EOF and is reaped.
        executable = os.stat(f"/proc/{owner.pid}/exe")
        helpers = []
        candidates = set()
        for task in Path(f"/proc/{owner.pid}/task").iterdir():
            candidates.update(int(pid) for pid in (task / "children").read_text().split())
        for pid in candidates:
            handle = None
            try:
                handle = os.pidfd_open(pid)
                arguments = Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0")[:-1]
                identity = os.stat(f"/proc/{pid}/exe")
                status = Path(f"/proc/{pid}/status").read_text().splitlines()
                parent = int(next(line.split()[1] for line in status if line.startswith("PPid:")))
                if parent == owner.pid and len(arguments) == 2 and arguments[1] == b"--internal-volume-monitor" and (identity.st_dev, identity.st_ino) == (executable.st_dev, executable.st_ino):
                    helpers.append((pid, handle))
                    handle = None
            except (FileNotFoundError, ProcessLookupError):
                pass
            finally:
                if handle is not None:
                    os.close(handle)
        if len(helpers) != 1:
            for _, handle in helpers:
                os.close(handle)
        assert len(helpers) == 1, helpers
        helper_pid, helper = helpers[0]
        try:
            signal.pidfd_send_signal(helper, signal.SIGSTOP)
            session.wait_for(lambda: Path(f"/proc/{helper_pid}/stat").read_text().rsplit(") ", 1)[1].startswith("T "), "exact private helper stopped")
            command("--quit")
            time.sleep(1)
            assert owner.poll() is None, "orderly Quit released the owner before its platform helper finished"
        finally:
            try:
                signal.pidfd_send_signal(helper, signal.SIGCONT)
            except ProcessLookupError:
                pass
            os.close(helper)
        assert owner.wait(timeout=10) == 0
        command("--quit")
        checks += ["actual --daemon and repeated activation run without a display environment",
                   "a browser request is refused while the same headless backend stays alive",
                   "--quit retains ownership through platform-helper teardown; a second Quit has no owner to start"]
        result.update(status="passed", checks=checks, scope="headless initial host and orderly lifetime; graphical reopening is a separate session case")
        return result

    def windows():
        return [w for w in json.loads(session.run(["niri", "msg", "--json", "windows"])) if w.get("app_id") == "org.canopa.Files"]

    def title(name):
        return next((w for w in windows() if w.get("title") == f"{name} — Canopa Files"), None)

    def focus(window):
        session.run(["niri", "msg", "action", "focus-window", "--id", str(window["id"])])
        session.wait_for(lambda: any(w["id"] == window["id"] and w["is_focused"] for w in windows()), "focus")
        time.sleep(.15)

    def key(name, *modifiers):
        args = ["wtype"]
        for modifier in modifiers:
            args += ["-M", modifier]
        args += ["-k", name]
        for modifier in reversed(modifiers):
            args += ["-m", modifier]
        session.run(args)
        time.sleep(.2)

    def opened(path):
        started = time.monotonic()
        command(path)
        browser = session.wait_for(lambda: live(lambda: title(root.name)), "activated Browser")
        assert browser["pid"] == owner.pid, browser
        session.wait_for(lambda: any(w["id"] == browser["id"] and w["is_focused"] for w in windows()), "activated Browser focus")
        return browser, time.monotonic() - started

    def close_browser(browser):
        focus(browser)
        key("w", "ctrl")
        session.wait_for(lambda: not windows(), "no Files windows")
        assert owner.poll() is None

    def notice(fragment):
        return session.wait_for(lambda: any(fragment in event["title"] for event in json.loads(events.read_text())), "operation completion attention")

    assert not windows()
    raw = os.fsencode(root) + b"/raw-\xff.txt"
    Path(os.fsdecode(raw)).write_bytes(b"Resident raw file contents\n")
    browser, result["first_open_seconds"] = opened(os.fsdecode(raw))
    key("F2")
    session.wait_for(lambda: title("Rename"), "selected raw-file Rename")
    session.run(["wtype", "Renamed.txt"])
    key("Return")
    session.wait_for(lambda: (root / "Renamed.txt").is_file() and not os.path.exists(raw), "actual raw-file Rename")
    notice("Renamed.txt")
    close_browser(browser)
    result["idle_after_gui"] = metrics(owner.pid)
    browser, warm = opened(root)
    result["warm_open_seconds"] = [warm]
    key("z", "ctrl")
    session.wait_for(lambda: os.path.exists(raw) and not (root / "Renamed.txt").exists(), "same-runtime Undo")
    assert Path(os.fsdecode(raw)).read_bytes() == b"Resident raw file contents\n"
    checks += ["later CLI opens a Browser in the exact resident process and preserves raw Unix filename selection",
               "closing every window keeps the owner; reactivation uses its shared Undo history"]
    time.sleep(.3)
    close_browser(browser)
    for _ in range(2):
        browser, warm = opened(root)
        result["warm_open_seconds"].append(warm)
        close_browser(browser)
    browser, _ = opened(root)
    key("n", "ctrl", "shift")
    session.wait_for(lambda: title("New folder"), "new folder prompt")
    session.run(["wtype", "Waiting review"])
    key("Return")
    folder = root / "Waiting review"
    session.wait_for(folder.is_dir, "actual created folder")
    notice("Waiting review")
    (folder / "later.txt").write_bytes(b"Later resident work survives\n")
    focus(browser)
    key("z", "ctrl")
    session.wait_for(lambda: title("Undo needs a decision"), "generation-bound Undo question")
    focus(browser)
    key("n", "ctrl", "shift")
    session.wait_for(lambda: title("New folder"), "queued folder prompt")
    session.run(["wtype", "Queued after review"])
    key("Return")
    session.wait_for(lambda: title("New folder") is None, "queued operation admission")
    queued = root / "Queued after review"
    assert not queued.exists()
    focus(browser)
    key("w", "ctrl")
    operations = session.wait_for(lambda: title("Operations"), "Operations after Browser closure")
    focus(operations)
    key("Escape")
    session.wait_for(lambda: not windows(), "all Browser, question and Operations windows closed")
    command("--quit")
    time.sleep(.4)
    assert owner.poll() is None and not queued.exists()
    assert (folder / "later.txt").read_bytes() == b"Later resident work survives\n"
    checks.append("an unanswered decision and queued mutation survive all windows closing and a requested orderly Quit")
    browser, _ = opened(root)
    focus(browser)
    key("j", "ctrl")
    operations = session.wait_for(lambda: title("Operations"), "reactivated shared Operations")
    focus(operations)
    # New browser, Recovery, queued Cancel/Open folder, Undo Cancel/Review.
    for _ in range(6):
        key("Tab")
    session.capture("resident-review-control.png")
    key("Return")
    question = session.wait_for(lambda: title("Undo needs a decision"), "same pending decision through Operations")
    session.capture("resident-review.png")
    focus(question)
    key("Tab")  # Keep item; initial focus is Cancel.
    key("Return")
    session.wait_for(queued.is_dir, "queued operation resumes after explicit review")
    notice("Queued after review")
    assert (folder / "later.txt").read_bytes() == b"Later resident work survives\n"
    focus(operations)
    key("Escape")
    focus(browser)
    key("w", "ctrl")
    assert owner.wait(timeout=10) == 0
    assert not windows()
    checks += ["reactivation restores the same unanswered question; an explicit Keep item releases queued work without changing later bytes",
               "released residency exits only after windows, work and notification holds settle"]
    result.update(status="passed", checks=checks, scope="private debug-build resident launch and activation; timings are fixture measurements, not cold-cache or release benchmarks")
    return result


def main():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--headless", action="store_true")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    binary = args.binary.resolve()
    with binary.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    result = {"status": "failed", "binary": str(binary), "sha256": digest}
    with tempfile.TemporaryDirectory(prefix="canopa-files-native-resident-") as private:
        runtime = Path(private)
        runtime.chmod(0o700)
        session = desktop.Session(runtime, output)
        try:
            result.update(verify(binary, session, args.headless))
        except Exception as error:
            result["error"] = str(error)
            if not args.headless:
                try:
                    result["windows"] = json.loads(session.run(["niri", "msg", "--json", "windows"]))
                    session.capture("failure.png")
                except Exception:
                    pass
            raise
        finally:
            session.close()
            (output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
