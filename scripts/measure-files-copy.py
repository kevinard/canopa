#!/usr/bin/env python3
"""Measure Canopa Files Copy throughput against cp and gio on private files.

Build the headless probe first, with the release profile in its own target dir:

    CARGO_TARGET_DIR=target/copy-throughput bash scripts/cargo.sh build \
        -p canopa-files --no-default-features --release --example copy_throughput --locked

Then run, for example:

    python3 scripts/measure-files-copy.py \
        --engine candidate=target/copy-throughput/release/examples/copy_throughput \
        --base ext4=target/copy-bench --base tmpfs=/dev/shm/canopa-copy-bench \
        --rounds 3 --output target/copy-bench/results.jsonl

Every base folder is created and owned by this script. Source trees are written
once per base and reused, so all tools read cache-warm sources; each copy writes a
fresh output folder that is removed afterwards. No caches are dropped. Tools:

- canopa:  the production engine, durable when it returns. Each --engine
           LABEL=PATH adds one "canopa:LABEL" tool, so a frozen baseline and a
           candidate alternate within every round under the same load.
- cp:      `cp -r`, which returns before data reaches the device.
- cp+sync: `cp -r` followed by `sync -f` on the destination filesystem.
- gio:     `gio copy` (one call per destination folder; it does not recurse).
- gio+sync: the same followed by `sync -f`.

With --probe, Canopa also runs once under an LD_PRELOAD shim that counts and
times fsync/fdatasync/renameat2/openat calls (scripts/fixtures/copy-sync-probe.c).
Outputs are JSON lines; a median summary is printed at the end.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
WORKLOADS = ("small100", "small10k", "large1g", "mixed")
TOOLS = ("canopa", "cp", "cp+sync", "gio", "gio+sync")


def payload(size, seed):
    # Cheap incompressible-enough bytes; content is irrelevant to ext4/tmpfs.
    block = (seed.to_bytes(8, "little") * 512)[:4096]
    data = bytearray(os.urandom(min(size, 4096)))
    while len(data) < size:
        data += block
    return bytes(data[:size])


def create(workload, path):
    """Create one source tree. Returns (items, bytes)."""
    path.mkdir(parents=True)
    items = total = 0
    if workload in ("small100", "small10k"):
        count = 100 if workload == "small100" else 10_000
        folder = path / "files"
        folder.mkdir()
        for n in range(count):
            data = payload(16 * 1024 if count == 100 else 4 * 1024, n)
            (folder / f"file-{n:05}.dat").write_bytes(data)
            items += 1
            total += len(data)
        items += 1
    elif workload == "large1g":
        chunk = os.urandom(8 * 1024 * 1024)
        with open(path / "large.bin", "wb") as file:
            for _ in range(128):
                file.write(chunk)
        items, total = 1, 1 << 30
    else:
        sizes = (1024, 4096, 16 * 1024, 64 * 1024, 256 * 1024)
        tree = path / "tree"
        for top in range(4):
            top_dir = tree / f"project-{top}"
            for sub in range(5):
                folder = top_dir / f"folder-{sub}"
                folder.mkdir(parents=True)
                for n in range(40):
                    data = payload(sizes[n % len(sizes)], n)
                    (folder / f"item-{n:03}.dat").write_bytes(data)
                    items += 1
                    total += len(data)
                items += 1
            big = payload(16 * 1024 * 1024, top)
            (top_dir / "media.bin").write_bytes(big)
            items += 2
            total += len(big)
        items += 1
    subprocess.run(["sync", "-f", str(path)], check=True)
    return items, total


def sources(workload, path):
    if workload == "large1g":
        return [path / "large.bin"]
    return [path / ("tree" if workload == "mixed" else "files")]


def gio_copy(source, destination):
    if source.is_file():
        subprocess.run(["gio", "copy", "-P", str(source), str(destination)], check=True)
        return
    for folder, dirs, files in os.walk(source):
        relative = Path(folder).relative_to(source.parent)
        target = destination / relative
        target.mkdir()
        if files:
            names = [str(Path(folder) / name) for name in sorted(files)]
            subprocess.run(["gio", "copy", "-P", *names, str(target)], check=True)


def run(tool, engines, workload, source_root, work, probe=None):
    output = Path(tempfile.mkdtemp(prefix="out-", dir=work))
    selected = sources(workload, source_root)
    record = {"tool": tool, "workload": workload}
    start = time.perf_counter()
    if tool.startswith("canopa"):
        binary = engines[tool]
        history = Path(tempfile.mkdtemp(prefix="history-", dir=work))
        env = dict(os.environ)
        if probe:
            report = work / "probe.jsonl"
            report.unlink(missing_ok=True)
            env.update(LD_PRELOAD=str(probe), COPY_SYNC_PROBE_OUT=str(report))
        line = subprocess.check_output([binary, history / "history", output, *selected], env=env, text=True)
        record["wall_s"] = time.perf_counter() - start
        record.update(json.loads(line.strip().splitlines()[-1]))
        record["engine_s"] = record.pop("elapsed_us") / 1e6
        if probe:
            record["probe"] = json.loads(report.read_text().strip().splitlines()[-1])
        shutil.rmtree(history)
    elif tool.startswith("cp"):
        subprocess.run(["cp", "-r", "-P", *map(str, selected), str(output)], check=True)
    else:
        for source in selected:
            gio_copy(source, output)
    if tool.endswith("+sync"):
        subprocess.run(["sync", "-f", str(output)], check=True)
    record.setdefault("wall_s", time.perf_counter() - start)
    shutil.rmtree(output)
    subprocess.run(["sync", "-f", str(work)], check=True)
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--engine", action="append", required=True, help="LABEL=PATH, repeatable")
    parser.add_argument("--base", action="append", required=True, help="LABEL=FOLDER, repeatable")
    parser.add_argument("--workloads", default=",".join(WORKLOADS))
    parser.add_argument("--tools", default=",".join(TOOLS))
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--probe", action="store_true", help="add one LD_PRELOAD syscall-count run")
    parser.add_argument("--label", default="", help="variant label stored in each record")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--keep-sources", action="store_true")
    parser.add_argument("--no-warmup", action="store_true", help="skip the warm-up copy per tool")
    args = parser.parse_args()
    engines = {}
    for engine in args.engine:
        label, _, path = engine.partition("=")
        engines[f"canopa:{label}"] = Path(path).resolve()
    workloads = args.workloads.split(",")
    tools = []
    for tool in args.tools.split(","):
        tools.extend(engines if tool == "canopa" else [tool])
    probe = None
    if args.probe:
        probe_dir = Path(tempfile.mkdtemp(prefix="canopa-copy-probe-"))
        probe = probe_dir / "probe.so"
        subprocess.run(["cc", "-O2", "-shared", "-fPIC", "-o", str(probe),
                        str(ROOT / "scripts/fixtures/copy-sync-probe.c"), "-ldl"], check=True)
    records = []
    out = args.output.open("a") if args.output else None
    for base in args.base:
        label, _, folder = base.partition("=")
        folder = Path(folder).resolve()
        folder.mkdir(parents=True, exist_ok=True)
        for workload in workloads:
            source_root = folder / f"source-{workload}"
            if not source_root.exists():
                items, size = create(workload, source_root)
                (folder / f"source-{workload}.json").write_text(json.dumps({"items": items, "bytes": size}))
            work = folder / "work"
            work.mkdir(exist_ok=True)
            plan = [(tool, False) for tool in tools]
            if probe:
                plan.extend((tool, True) for tool in tools if tool in engines)
            # Warm up once, then alternate tools in each round.
            for tool, _ in [] if args.no_warmup else plan[: len(tools)]:
                run(tool, engines, workload, source_root, work)
            for round_ in range(args.rounds):
                for tool, probed in plan:
                    if probed and round_ > 0:
                        continue
                    record = run(tool, engines, workload, source_root, work, probe if probed else None)
                    record.update(filesystem=label, round=round_, variant=args.label, probed=probed)
                    records.append(record)
                    line = json.dumps(record, sort_keys=True)
                    print(line, flush=True)
                    if out:
                        out.write(line + "\n")
                        out.flush()
            if not args.keep_sources:
                shutil.rmtree(source_root)
        shutil.rmtree(folder / "work", ignore_errors=True)
    print("\nMedian wall seconds (unprobed runs):")
    keys = sorted({(r["filesystem"], r["workload"], r["tool"]) for r in records if not r["probed"]})
    for key in keys:
        values = [r["wall_s"] for r in records if not r["probed"] and (r["filesystem"], r["workload"], r["tool"]) == key]
        print(f"  {key[0]:6} {key[1]:9} {key[2]:18} {statistics.median(values):9.3f}  (n={len(values)})")
    for r in records:
        if r["probed"]:
            print(f"  probe {r['filesystem']} {r['workload']} {r['tool']}: {json.dumps(r['probe'])}")


if __name__ == "__main__":
    main()
