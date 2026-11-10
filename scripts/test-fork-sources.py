#!/usr/bin/env python3
"""Check fork source pins, archive verification and prepared-source reuse without builds or downloads."""
import importlib.util
import io
import json
from pathlib import Path
import re
import sys
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import fork_sources  # noqa: E402


def module(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), ROOT / "scripts" / f"{name}.py")
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


niri_builder = module("build-niri")
portal_builder = module("build-xdg-portal-compat")


def write_archive(path, root, files, extra=None):
    with tarfile.open(path, "w:gz") as archive:
        for name, data in files.items():
            info = tarfile.TarInfo(f"{root}/{name}")
            info.size = len(data)
            archive.addfile(info, io.BytesIO(data))
        if extra:
            archive.addfile(extra)


class Fixture(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="canopa-fork-sources-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.archives = self.root / "archives"
        self.archives.mkdir()
        self.commit = "1" * 40
        self.files = {"LICENSE": b"Original license\n", "src/lib.rs": b"fn main() {}\n", "Cargo.lock": b"version = 4\n"}
        self.archive = self.archives / f"demo-{self.commit}.tar.gz"
        write_archive(self.archive, f"demo-{self.commit}", self.files)
        self.manifest = self.root / "forks.json"
        self.pin = dict(repository="https://github.com/kevinard/demo", upstream="https://github.com/example/demo",
                        branch="canopa/main", commit=self.commit, base="0" * 40,
                        url=f"https://codeload.github.com/kevinard/demo/tar.gz/{self.commit}",
                        archive=self.archive.name, archive_sha256=fork_sources.file_digest(self.archive),
                        archive_root=f"demo-{self.commit}",
                        notices={"LICENSE": fork_sources.digest(self.files["LICENSE"])})
        self.manifest.write_text(json.dumps({"demo": self.pin}))


class ArchiveTests(Fixture):
    def test_pinned_archive_is_verified_and_extracted(self):
        record, archive = fork_sources.origin("demo", self.archives, manifest=self.manifest)
        self.assertTrue(record["pinned"])
        self.assertEqual(record["ref"], "canopa/main")
        fork_sources.check_origin(record, "demo", self.manifest)
        source = fork_sources.extract(archive, self.root / "work", record["archive_root"])
        self.assertEqual(fork_sources.inventory(source),
                         fork_sources.archive_inventory(archive, record["archive_root"]))
        self.assertEqual(set(fork_sources.notices(source, self.pin["notices"])), {"LICENSE"})

    def test_changed_or_missing_archive_is_refused_without_downloading(self):
        self.archive.write_bytes(self.archive.read_bytes() + b"\0")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            fork_sources.origin("demo", self.archives, manifest=self.manifest)
        self.archive.unlink()
        with self.assertRaisesRegex(ValueError, "--download"):
            fork_sources.origin("demo", self.archives, manifest=self.manifest)

    def test_unpinned_origin_and_changed_notice_are_refused(self):
        record, _ = fork_sources.origin("demo", self.archives, manifest=self.manifest)
        for key, value in [("commit", "2" * 40), ("archive_sha256", "0" * 64), ("pinned", False)]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                fork_sources.check_origin({**record, key: value}, "demo", self.manifest)
        source = fork_sources.extract(self.archive, self.root / "work", record["archive_root"])
        (source / "LICENSE").write_text("Replaced\n")
        with self.assertRaisesRegex(ValueError, "differ from the pinned fork"):
            fork_sources.notices(source, self.pin["notices"])

    def test_unsafe_archive_members_are_refused(self):
        link = tarfile.TarInfo(f"demo-{self.commit}/alias")
        link.type = tarfile.SYMTYPE
        link.linkname = "/etc/passwd"
        for name, extra in [("link", link), ("outside", tarfile.TarInfo("elsewhere/file"))]:
            with self.subTest(name=name):
                archive = self.root / f"{name}.tar.gz"
                write_archive(archive, f"demo-{self.commit}", self.files, extra)
                with self.assertRaises(ValueError):
                    fork_sources.extract(archive, self.root / name, f"demo-{self.commit}")

    def test_prefix_selection_extracts_only_named_directories(self):
        archive = self.root / "release.tar.gz"
        write_archive(archive, "release", {"subprojects/gvdb/a.c": b"a", "subprojects/gvdb.wrap": b"w", "README": b"r"})
        self.assertEqual(set(fork_sources.archive_inventory(archive, "release", ["subprojects/gvdb"])),
                         {"subprojects/gvdb/a.c"})


class NiriReuseTests(Fixture):
    def prepared(self):
        record, archive = fork_sources.origin("demo", self.archives, manifest=self.manifest)
        work = self.root / "work"
        source = fork_sources.extract(archive, work, record["archive_root"])
        inputs = fork_sources.inventory(source)
        receipt = {"fork": "niri", "origin": record, "source": str(source), "source_file_count": len(inputs),
                   "source_inputs_sha256": fork_sources.write_inputs(work, inputs), "status": "built"}
        (work / "canopa-build.json").write_text(json.dumps(receipt))
        return work, source, record, receipt

    def test_unchanged_reuse_preserves_paths_contents_and_mtimes(self):
        work, source, record, receipt = self.prepared()
        before = {str(path): path.stat().st_mtime_ns for path in source.rglob("*")}
        niri_builder.reuse_prepared(work, source, record, receipt)
        self.assertEqual(before, {str(path): path.stat().st_mtime_ns for path in source.rglob("*")})

    def test_changed_removed_added_and_linked_inputs_are_refused(self):
        for change in ("edit", "remove", "add", "symlink"):
            with self.subTest(change=change):
                work, source, record, receipt = self.prepared()
                path = source / "src/lib.rs"
                if change == "edit":
                    path.write_text("unexpected source")
                elif change == "remove":
                    path.unlink()
                elif change == "add":
                    path.with_name("extra.rs").write_text("extra")
                else:
                    path.with_name("alias.rs").symlink_to(path)
                with self.assertRaises(ValueError):
                    niri_builder.reuse_prepared(work, source, record, receipt)
                for item in sorted(work.rglob("*"), reverse=True):
                    item.unlink() if item.is_file() or item.is_symlink() else item.rmdir()

    def test_other_commit_or_location_is_refused(self):
        work, source, record, receipt = self.prepared()
        with self.assertRaises(ValueError):
            niri_builder.reuse_prepared(work, source, {**record, "commit": "2" * 40}, receipt)
        with self.assertRaises(ValueError):
            niri_builder.reuse_prepared(work, source.with_name("other"), record, receipt)


class PinTests(unittest.TestCase):
    def test_lockfile_git_sources_must_be_the_pinned_smithay_fork(self):
        smithay = fork_sources.pin("niri")["cargo_git"]
        commit = smithay["smithay"]["commit"]
        lock = "".join(f'[[package]]\nname = "{name}"\nversion = "0.1.0"\n'
                       f'source = "git+https://github.com/kevinard/smithay.git?rev={commit}#{commit}"\n\n'
                       for name in ("smithay", "smithay-drm-extras"))
        niri_builder.cargo_git_sources(lock, smithay)
        for changed in [lock.replace("kevinard/smithay", "Smithay/smithay"), lock.replace(f"#{commit}", "#" + "0" * 40),
                        lock + '[[package]]\nname = "other"\nversion = "1.0.0"\nsource = "git+https://example.org/other#' + "0" * 40 + '"\n']:
            with self.subTest(changed=changed[-60:]), self.assertRaises(ValueError):
                niri_builder.cargo_git_sources(changed, smithay)

    def test_arch_recipe_downloads_exactly_the_pinned_archives(self):
        recipe = (ROOT / "packaging/arch/PKGBUILD.in").read_text()
        variables = dict(re.findall(r"^(_\w+)=(\S+)$", recipe, re.M))
        source = re.search(r"^source=\((.*?)\)$", recipe, re.M | re.S)[1]
        checksums = re.findall(r"'([0-9a-f]{64}|@SOURCE_SHA256@)'", re.search(r"^sha256sums=\((.*?)\)$", recipe, re.M | re.S)[1])
        entries = [re.sub(r"\$(_\w+)", lambda match: variables[match[1]], entry)
                   for entry in re.findall(r"['\"]([^'\"]+)['\"]", source)]
        pins = fork_sources.pins()
        portal = pins["xdg-desktop-portal"]
        expected = ["canopa-@VERSION@.tar.gz",
                    f"{pins['niri']['archive']}::{pins['niri']['url']}",
                    f"{portal['archive']}::{portal['url']}",
                    portal["subprojects"]["url"]]
        self.assertEqual(entries, expected)
        self.assertEqual(checksums, ["@SOURCE_SHA256@", pins["niri"]["archive_sha256"], portal["archive_sha256"],
                                     portal["subprojects"]["archive_sha256"]])
        self.assertEqual(portal["subprojects"]["archive"], portal["subprojects"]["url"].rsplit("/", 1)[1])
        self.assertNotIn("vendor", recipe)

    def test_every_pin_is_a_commit_archive_of_its_fork(self):
        for name, value in fork_sources.pins().items():
            with self.subTest(name=name):
                self.assertRegex(value["commit"], r"^[0-9a-f]{40}$")
                self.assertRegex(value["base"], r"^[0-9a-f]{40}$")
                self.assertTrue(value["repository"].startswith("https://github.com/kevinard/"))
                slug = value["repository"].removeprefix("https://github.com/")
                self.assertEqual(value["url"], f"https://codeload.github.com/{slug}/tar.gz/{value['commit']}")
                self.assertEqual(value["archive"], f"{name}-{value['commit']}.tar.gz")
                self.assertEqual(value["archive_root"], f"{name}-{value['commit']}")
                self.assertRegex(value["archive_sha256"], r"^[0-9a-f]{64}$")
                self.assertTrue(value["notices"])

    def test_portal_subprojects_cannot_replace_fork_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fork = root / "fork.tar.gz"
            release = root / "release.tar.gz"
            write_archive(fork, "fork", {"meson.build": b"project()\n", "subprojects/gvdb.wrap": b"[wrap-git]\n"})
            write_archive(release, "release", {"meson.build": b"project()\n", "subprojects/gvdb/gvdb.c": b"gvdb\n"})
            spec = {"archive_root": "release", "paths": ["subprojects/gvdb"]}
            self.assertEqual(set(portal_builder.expected_inputs(fork, {"archive_root": "fork"}, spec, release)),
                             {"meson.build", "subprojects/gvdb.wrap", "subprojects/gvdb/gvdb.c"})
            with self.assertRaises(ValueError):
                portal_builder.expected_inputs(fork, {"archive_root": "fork"}, {**spec, "paths": ["meson.build"]}, release)


if __name__ == "__main__":
    unittest.main()
