#!/usr/bin/env python3
"""Private real-compositor clipboard transport; no portal consent/routing claim."""
import argparse
import json
from pathlib import Path
import sys
import subprocess
import tempfile
import time
import traceback

sys.dont_write_bytecode = True
from screencast_media import MediaSession, digest

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("harness", "niri"):
        parser.add_argument("--" + name, type=Path, required=True)
        parser.add_argument("--" + name + "-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    for name in ("harness", "niri"):
        path = getattr(args, name).resolve(strict=True)
        setattr(args, name, path)
        if digest(path) != getattr(args, name + "_sha256"):
            parser.error(name + " checksum mismatch")
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=False)
    sources = [Path(__file__), ROOT / "scripts/screencast_media.py",
               ROOT / "scripts/validate-desktop.py", args.harness, args.niri]
    pins = {str(path): digest(path) for path in sources}
    result = {"status": "running", "scope": __doc__, "pins": pins}
    started = time.monotonic()
    with tempfile.TemporaryDirectory(prefix="canopa-clipboard-transport-") as directory:
        session = MediaSession(Path(directory), args.output, args.niri, args.niri_sha256,
                               reference_frozen=True)
        try:
            session.start_niri(
                "debug { dbus-interfaces-in-non-session-instances; }\n"
                "animations { off; }\nxwayland-satellite { off; }\n"
                "hotkey-overlay { skip-at-startup; }\n", scale=1.25)
            niri = next(process for process in session.processes
                        if process.args[0] == str(session.compositor))
            session.env.update(CANOPA_CLIPBOARD_NIRI_PID=str(niri.pid),
                               CANOPA_CLIPBOARD_OUTPUT=str(args.output))
            session.env.pop("WAYLAND_SOCKET", None)
            # Direct compiled harnesses use the crate working directory, as Cargo does.
            with (args.output / "harness.log").open("w") as log:
                process = subprocess.Popen([
                    str(args.harness), "--ignored", "--exact",
                    "clipboard_transport::tests::native_clipboard_transport", "--nocapture",
                ], cwd=ROOT / "crates/canopa-portal", env=session.env,
                    stdout=log, stderr=log, start_new_session=True)
                session.processes.append(process)
                code = process.wait(timeout=90)
            result["harness_exit"] = code
            assert code == 0, "Native transport harness failed; see harness.log"
            result.update(json.loads((args.output / "native-result.json").read_text()))
            assert result["status"] == "passed"
        except Exception as error:
            result.update(status="failed", error=repr(error), traceback=traceback.format_exc())
        finally:
            processes = list(session.processes)
            session.close()
            result["processes"] = [{"pid": process.pid, "command": process.args,
                                    "exit_code": process.poll()} for process in processes]
    result.update(seconds=round(time.monotonic() - started, 2),
                  changed_inputs=[path for path, checksum in pins.items()
                                  if digest(Path(path)) != checksum])
    if result["changed_inputs"] or any(row["exit_code"] is None for row in result["processes"]):
        result["status"] = "failed"
    (args.output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({key: result.get(key) for key in ("status", "error", "harness_exit", "seconds")}))
    raise SystemExit(0 if result["status"] == "passed" else 1)


if __name__ == "__main__":
    main()
