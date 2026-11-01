#!/usr/bin/env python3
"""Stage release documentation with working offline links, or check a bundle."""

import argparse
import os
import re
import shutil
from pathlib import Path
from urllib.parse import quote, unquote, urlsplit, urlunsplit

ROOT = Path(__file__).resolve().parent.parent
FILES = {
    "README.md": "share/canopa/README.md",
    "LICENSES.md": "share/canopa/LICENSES.md",
    "config/canopa.kdl": "share/canopa/canopa.kdl",
    **{f"docs/{name}.md": f"share/canopa/{name}.md" for name in (
        "settings-portal", "shell-rename", "audio", "media", "files",
        "notifications", "configuration", "theme-customization", "session-tools", "greeter", "greeter-performance", "polkit", "pinentry",
    )},
    **{f"examples/{name}": f"share/canopa/examples/{name}" for name in (
        "canopa-desktop.kdl", "custom-theme.kdl", "themes/nord.kdl",
    )},
    **{f"crates/canopa-{package}/{name}": f"share/canopa-{package}/{name}"
       for package, names in (
           ("shell", ("README.md", "LICENSE")),
           ("notify", ("README.md", "LICENSE-MIT", "LICENSE-APACHE")),
           ("files", ("README.md", "LICENSE-MIT", "LICENSE-APACHE")),
           ("session", ("README.md", "LICENSE-MIT", "LICENSE-APACHE")),
           ("session-ui", ("README.md", "LICENSE-MIT", "LICENSE-APACHE")),
           ("greeter", ("README.md", "LICENSE-MIT", "LICENSE-APACHE")),
           ("pinentry", ("README.md", "LICENSE-MIT", "LICENSE-APACHE")),
           ("polkit", ("README.md", "LICENSE-MIT", "LICENSE-APACHE")),
           ("widgets", ("README.md", "LICENSE")),
       ) for name in names},
    "crates/canopa-shell/config/default.kdl": "share/canopa-shell/default.kdl",
    "crates/canopa-notify/examples/config.kdl": "share/canopa-notify/config.kdl",
}
LINK = re.compile(r"(?P<image>!?)\[(?P<label>[^\]\n]*)\]\((?P<url>[^)\n]+)\)")
FENCE = re.compile(r"(^```[^\n]*\n.*?^```[^\n]*(?:\n|$))", re.M | re.S)


def prose(text, replace):
    """Leave fenced examples intact while processing the guides' inline links."""
    return "".join(part if index % 2 else LINK.sub(replace, part)
                   for index, part in enumerate(FENCE.split(text)))


def stage(bundle):
    destinations = {(ROOT / source).resolve(): Path(target)
                    for source, target in FILES.items()}
    for source, target in FILES.items():
        source = ROOT / source
        output = bundle / target
        output.parent.mkdir(parents=True, exist_ok=True)
        if source.suffix != ".md":
            shutil.copyfile(source, output)
            output.chmod(0o644)
            continue

        def rewrite(match):
            url = urlsplit(match["url"])
            if url.scheme or url.netloc or not url.path:
                return match[0]
            resolved = (source.parent / unquote(url.path)).resolve()
            if not resolved.is_relative_to(ROOT) or not resolved.exists():
                raise ValueError(f"{source.relative_to(ROOT)}: missing source reference {url.path}")
            destination = destinations.get(resolved)
            if destination is None:
                if match["image"]:
                    raise ValueError(f"{source.relative_to(ROOT)}: unshipped image {url.path}")
                return f'{match["label"]} (source checkout: `{resolved.relative_to(ROOT)}`)'
            relative = quote(os.path.relpath(destination, Path(target).parent), safe="/.-_")
            url = urlunsplit(("", "", relative, url.query, url.fragment))
            return f'{match["image"]}[{match["label"]}]({url})'

        output.write_text(prose(source.read_text(), rewrite))
        output.chmod(0o644)
    check(bundle)


def check(bundle):
    count = 0
    for target in FILES.values():
        path = bundle / target
        if not path.is_file():
            raise ValueError(f"Missing packaged documentation: {target}")
        if path.suffix != ".md":
            continue

        def verify(match):
            nonlocal count
            url = urlsplit(match["url"])
            if url.scheme or url.netloc or not url.path:
                return match[0]
            resolved = (path.parent / unquote(url.path)).resolve()
            if not resolved.is_relative_to(bundle) or not resolved.is_file():
                raise ValueError(f"{target}: broken packaged link {match['url']}")
            count += 1
            return match[0]

        prose(path.read_text(), verify)
    print(f"Release documentation: {len(FILES)} files; {count} offline links resolve.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    bundle = args.bundle.resolve()
    if args.check:
        check(bundle)
    else:
        stage(bundle)


if __name__ == "__main__":
    main()
