#!/usr/bin/env python3
"""Verify command or filesystem attention on a private bus without any GUI."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def test_binary(output, workflow):
    target = {"actions": "local_actions", "operations": "local_operation_attention", "devices": "device_runtime"}[workflow]
    feature = "local-runtime" if workflow == "devices" else f"background-{workflow}"
    command = ["cargo", "test", "--locked", "--offline", "-j1", "-p", "canopa-files",
               "--no-default-features", "--features", feature, "--test", target,
               "--no-run", "--message-format=json"]
    # Compile before starting the private bus, as in clipboard acceptance.
    built = subprocess.run(command, cwd=ROOT, text=True, capture_output=True,
                           env={**os.environ, "CARGO_INCREMENTAL": "0"})
    (output / "build.jsonl").write_text(built.stdout)
    (output / "build.log").write_text(built.stderr)
    built.check_returncode()
    artifacts = [json.loads(line) for line in built.stdout.splitlines() if line.startswith("{")]
    assert not any(item.get("target", {}).get("name", "").startswith("iced") for item in artifacts)
    features = [item["features"] for item in artifacts if item.get("reason") == "compiler-artifact"
                and item.get("target", {}).get("name") == "canopa_files" and "lib" in item["target"]["kind"]]
    expected = ["background-actions", "background-operations", "local-runtime"] if workflow == "devices" else [feature]
    assert features == [expected], features
    return Path(next(item["executable"] for item in reversed(artifacts)
                     if item.get("reason") == "compiler-artifact"
                     and item.get("target", {}).get("name") == target
                     and item.get("executable")))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--test-binary", type=Path)
    parser.add_argument("--workflow", choices=("actions", "operations", "devices"), default="actions")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    operations = args.workflow == "operations"
    devices = args.workflow == "devices"
    binary = (args.test_binary or test_binary(output, args.workflow)).resolve()
    with binary.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    result = {"status": "failed", "binary": str(binary), "sha256": digest}
    with tempfile.TemporaryDirectory(prefix="canopa-action-host-") as directory:
        runtime = Path(directory)
        runtime.chmod(0o700)
        (runtime / "home").mkdir()
        (runtime / "fixture-owned").write_bytes(b"Canopa private device attention\n" if devices else b"Canopa private operation attention\n" if operations else b"Canopa private action host\n")
        session = desktop.Session(runtime, output)
        session.env.update(HOME=str(runtime / "home"))
        session.env["CANOPA_DEVICE_ATTENTION_FIXTURE" if devices else "CANOPA_OPERATION_ATTENTION_FIXTURE" if operations else "CANOPA_ACTION_HOST_FIXTURE"] = str(runtime)
        try:
            session.start_private_bus()
            events = runtime / "notifications.json"
            session.start([sys.executable, str(ROOT / "scripts/desktop-fixture/files-notifications.py"),
                           str(events), "--release", str(runtime / "release-notifications")], "notifications.log")
            session.wait_for(events.with_suffix(".ready").exists, "private notification receiver")
            helper = ("operation_delivery_and_decisions_hold_shutdown_without_gui_initialization" if operations
                      else "notification_delivery_is_bounded_and_holds_shutdown_without_gui_initialization")
            log = "devices.log" if devices else "operations.log" if operations else "actions.log"
            command = [str(binary)] if devices else [str(binary), "--ignored", "--exact", helper, "--test-threads=1"]
            process = session.start(command, log)
            status = process.wait(timeout=20)
            assert status == 0, (output / log).read_text()
            delivered = json.loads(events.read_text())
            report = json.loads((runtime / "result.json").read_text())
            if devices:
                assert report == {"completed": 2, "delivery_hold_refused_shutdown": True,
                                  "question_required_explicit_answer": True, "original_bytes_preserved": True}
                assert len(delivered) == 3, delivered
                assert all(event["app"] == "Canopa Files" for event in delivered)
                assert [(event["title"], event["body"]) for event in delivered] == [
                    ("Remove Fixture USB", "Device unmounted."),
                    ("Device needs a decision", "Fixture USB · Open Operations to review."),
                    ("Mount Fixture USB", "Device mounted."),
                ], delivered
                assert "private-secret" not in (output / log).read_text()
                assert "private-secret" not in json.dumps(delivered)
            elif operations:
                assert report["completed"] == 3 and report["decision_generation"] > 0
                assert report["delivery_hold_refused_shutdown"]
                assert len(delivered) == 4, delivered
                assert all(event["app"] == "Canopa Files" for event in delivered)
                assert delivered[0]["title"] == "create folder Headless operation" and delivered[0]["body"] == "Completed."
                assert delivered[1]["title"] == "Operation needs a decision" and delivered[1]["body"] == "Undo · Open Operations to review."
                assert delivered[2]["title"] == "create folder Headless operation" and "remaining" in delivered[2]["body"]
                assert delivered[3]["title"] == "create folder Queued without a browser" and delivered[3]["body"] == "Completed."
                assert (runtime / "Headless operation/later.txt").read_bytes() == b"Headless later work survives\n"
                assert (runtime / "Queued without a browser").is_dir()
            else:
                assert report["accepted"] == 20 and report["busy"] >= 3
                assert 1 <= len(delivered) <= 17
                assert len(delivered) + report["busy"] == 20
                assert sorted(event["title"] for event in delivered) == sorted(report["expected_titles"])
                assert all(event["app"] == "Canopa Files" and event["body"] == "Finished" for event in delivered)
                for index in range(20):
                    assert (runtime / f"command-{index}/completed").read_bytes() == f"headless result {index}\n".encode()
            (output / "result.json").write_bytes((runtime / "result.json").read_bytes())
            assert not any(key in session.env for key in ("DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET"))
            if devices:
                result.update(status="passed", checks=[
                    "real private GIO mount/removal and device attention run without a GUI or frontend subscription",
                    "a held desktop notification reply retains backend ownership and refuses orderly shutdown",
                    "a password question reports attention and waits for its exact explicit answer without saving credentials",
                    "three actual notifications settle before shutdown; private device bytes survive and credentials stay out of logs"],
                    scope="headless device attention and delivery lifetime; no Iced device controls or physical-device acceptance claim")
            elif operations:
                result.update(status="passed", checks=[
                    "actual filesystem operations and Undo run with no GUI, frontend or observer",
                    "a real held Notify reply protects even direct core shutdown after the filesystem result completes",
                    "an unanswered generation-bound decision reports desktop attention and preserves later work with no browser",
                    "explicit review resumes queued work and all four actual notifications settle before orderly shutdown"],
                    scope="filesystem attention host; no daemon launch, primary-instance activation or restart continuation claim")
            else:
                result.update(status="passed", checks=[
                    "twenty real accepted commands complete with no GUI initialization, frontend or observer",
                    "stalled attention keeps at most seventeen deliveries and reports busy failures without losing command results",
                    "the runtime keeps its hold and refuses shutdown while the private notification reply is pending",
                    "actual standard desktop notification delivery releases the hold and permits orderly shutdown"],
                    scope="background command/attention workflow; no daemon launch, activation or filesystem attention claim")
        except Exception as error:
            result["error"] = str(error)
            raise
        finally:
            if (runtime / "notifications.json").exists():
                (output / "notifications.json").write_bytes((runtime / "notifications.json").read_bytes())
            session.close()
            (output / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
