#!/usr/bin/env python3
"""Stop only the private Files process after its real Undo quarantine appears."""
import json
import os
from pathlib import Path
import signal
import sys
import tempfile
import time

pid = int(sys.argv[1])
directory, evidence = map(Path, sys.argv[2:])
home = Path(os.environ["HOME"])
assert home.parent.name.startswith("canopa-files-native-") and home.parent.parent == Path(tempfile.gettempdir())
assert directory.is_relative_to(home) or directory.is_relative_to(home.parent / "volumes")
deadline = time.monotonic() + 45
while time.monotonic() < deadline:
    for payload in directory.glob(".canopa-files-stage-*/folder"):
        if payload.is_file() and payload.stat().st_size > 0:
            environment = Path(f"/proc/{pid}/environ").read_bytes().split(b"\0")
            assert os.fsencode(f"HOME={home}") in environment
            os.kill(pid, signal.SIGKILL)
            evidence.write_text(json.dumps({"pid": pid, "quarantine": str(payload), "signal": "SIGKILL"}) + "\n")
            sys.exit(0)
    time.sleep(.00005)
raise TimeoutError("did not observe the private Files Undo quarantine")
