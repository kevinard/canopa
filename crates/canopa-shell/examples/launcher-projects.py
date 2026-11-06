#!/usr/bin/env python3
"""Launcher extension: search immediate project folders under one explicit root."""
import hashlib
import json
from pathlib import Path
import sys


def main():
    root = Path(sys.argv[1]).resolve(strict=True)
    request = json.loads(sys.stdin.buffer.readline(4096))
    if request.get("protocol_version") != 1:
        raise ValueError("unsupported launcher protocol")
    query = request["query"].casefold()
    limit = min(int(request["limit"]), 20)
    matches = sorted(path for path in root.iterdir()
                     if not path.name.startswith(".") and path.is_dir()
                     and query in path.name.casefold())
    results = [{"id": hashlib.sha256(bytes(path)).hexdigest(), "label": path.name,
                "description": f"Project · {path.parent}", "icon": "folder",
                "actions": [{"label": "Open", "action": {"type": "open", "path": str(path)}},
                            {"label": "Copy path", "action": {"type": "copy", "text": str(path)}}]}
               for path in matches[:limit]]
    print(json.dumps({"protocol_version": 1, "results": results, "more": len(matches) > limit}))


if __name__ == "__main__":
    main()
