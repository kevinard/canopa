#!/usr/bin/env python3
"""Exercise shell reload on a private Niri desktop; retain screenshots and logs."""
import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("desktop", ROOT / "scripts/validate-desktop.py")
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


def configuration(edge="top", width=48, label="Before", target="all", scheme=None):
    shell = f'''bar "main" {{
    output {json.dumps(target)}
    edge "{edge}"
    collapsed-width {width}
    modules {{ center "button"; }}
}}
modules {{ module "button" {{ label "{label}"; icon "B"; actions {{ click {{ toggle-panel; }} }} }}; }}
'''
    if scheme:
        return f'schema-version 1\nappearance {{ color-scheme "{scheme}"; }}\nshell {{\n{shell}}}\n'
    return shell


def verify(binary, session):
    session.start_niri("prefer-no-csd\nanimations { off; }\nxwayland-satellite { off; }\n"
                       "hotkey-overlay { skip-at-startup; }\n")
    legacy = session.runtime / "config/niribar/config.kdl"
    common = session.runtime / "config/canopa/config.kdl"
    legacy.parent.mkdir(parents=True)
    common.parent.mkdir(parents=True)
    legacy.write_text(configuration())
    session.env.update(RUST_LOG="canopa_shell::configuration=info,canopa_shell::surface=debug")
    shell = session.start([str(binary)], "shell.log")
    session.wait_for_layers(1)
    log = lambda: (session.output / "shell.log").read_text()
    initial = session.capture("initial.png")
    initial_color = initial.getpixel((10, 10))

    def accepted_after(mark):
        session.wait_for(lambda: "configuration reloaded" in log()[mark:], "automatic reload")

    def save(content):
        mark = len(log())
        replacement = common.with_suffix(".new")
        replacement.write_text(content)
        replacement.replace(common)
        accepted_after(mark)

    # Appearance and content change in place, preserving the existing bar window.
    opens = log().count("opening bar surface")
    save(configuration(label="After", scheme="light"))
    assert log().count("opening bar surface") == opens
    session.wait_for(lambda: session.capture("light.png").getpixel((10, 10)) != initial_color,
                     "appearance repaint")
    light = session.capture("light.png")
    assert min(light.getpixel((10, 10))) > 150
    assert initial.crop((450, 0, 800, 48)).tobytes() != light.crop((450, 0, 800, 48)).tobytes()

    # The explicit command acknowledges installation and no-op reload leaves a panel open.
    session.run([str(binary), "--toggle-panel"])
    session.wait_for_layers(2)
    assert "reloaded" in session.run([str(binary), "reload"])
    assert len(session.layers()) == 2
    mark = len(log())
    common.write_text('schema-version 1\nshell { private-secret "bad"; }\n')
    session.wait_for(lambda: "reload rejected" in log()[mark:], "invalid edit rejection")
    result = subprocess.run([str(binary), "reload"], env=session.env, text=True, capture_output=True, timeout=10)
    assert result.returncode == 1 and "rejected" in result.stderr
    assert "private-secret" not in result.stderr + log()
    assert len(session.layers()) == 2, "invalid candidate must retain the open panel"

    # Geometry changes close foreground and replace the bar without restarting the process.
    save(configuration(edge="bottom", width=64, label="Moved", scheme="light"))
    session.wait_for_layers(1)
    session.wait_for(lambda: min(session.capture("bottom.png").getpixel((10, 790))) > 150,
                     "bottom bar geometry")
    assert shell.poll() is None

    # Keep observing outputs even while there are no matching surfaces.
    save(configuration(target="not-an-output", scheme="dark"))
    session.wait_for_layers(0)
    save(configuration(edge="left", width=72, scheme="light"))
    session.wait_for_layers(1)
    session.wait_for(lambda: min(session.capture("left.png").getpixel((10, 400))) > 150,
                     "recreated left bar")

    # Discovery reselects legacy input after removal of the common file.
    mark = len(log())
    common.unlink()
    accepted_after(mark)
    session.wait_for(lambda: session.capture("legacy-again.png").getpixel((10, 10)) == initial_color,
                     "legacy rediscovery")
    assert shell.poll() is None
    session.retire(shell)

    # An explicit path never falls back when it disappears.
    common.write_text(configuration(edge="bottom", scheme="light"))
    shell = session.start([str(binary), "--canopa-config", str(common)], "explicit.log")
    session.wait_for_layers(1)
    common.unlink()
    result = subprocess.run([str(binary), "reload"], env=session.env, text=True, capture_output=True, timeout=10)
    assert result.returncode == 1
    assert len(session.layers()) == 1 and shell.poll() is None
    session.capture("explicit-missing.png")
    desktop.write_json(session.output / "result.json", {"passed": True, "checks": [
        "atomic save", "appearance and content repaint", "stable bar window", "explicit reload",
        "no-op preserves panel", "invalid edit preserves panel and active state", "safe diagnostics",
        "geometry replacement", "zero matching outputs and recovery", "legacy rediscovery",
        "missing explicit file preserves active state", "shell process survives reloads",
    ]})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/canopa-shell")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix="canopa-reload-desktop-") as directory:
        session = desktop.Session(Path(directory), output)
        try:
            verify(args.binary.resolve(strict=True), session)
        finally:
            session.close()
    print(f"Configuration reload desktop checks passed: {output}")


if __name__ == "__main__":
    main()
