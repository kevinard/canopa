# 0167: Thumbnails in a sandbox

Status: implemented with core, sandbox and Desktop model verification,
2026-11-11. Native acceptance in a session is still pending. Fills the
`FileVisual` seam that ADR 0138 left for thumbnails, using the shared cache of
[root ADR 0141](../../adr/0141-mime-applications-and-thumbnail-cache.md).

## What is shown

The Icons, List and Columns views show a thumbnail before the type icon, keyed
by path and modification time, fitted into the icon square. Only rows that a tab
shows (the virtualized range plus its margin) are requested. At most three
thumbnails are prepared at once; up to 512 wait. Scrolling replaces the waiting
list. Leaving a folder (no active tab shows it) cancels its waiting and running
work. A changed file gets a new thumbnail. At most 4096 prepared thumbnails stay
in memory.

## Settings

The `files` block gains, written only when it differs from the default:

```kdl
thumbnails show="local" size="medium"
```

- `show`: `local` (default) skips network shares and removable media; `always`
  includes them; `never` turns thumbnails off.
- `size`: `small`, `medium` (default), `large` or `huge`: 48, 64, 96 or 128 pixel
  Icons-view icons. Medium replaces the earlier fixed 48 pixels. Thumbnails use
  the cache size covering twice the icon size (`normal` or `large`).

Both are in Preferences → Views. Placement comes from `/proc/self/mountinfo`
(network filesystem types, `//server` sources), sysfs (`removable`, USB, MMC or
FireWire bus) and `/media`, `/run/media` mount points. An unknown mount counts as
foreign.

## Preparation

`crate::thumbnails::Generator` runs on the worker, in this order:

1. Refuse files inside the thumbnail cache, and files the policy excludes.
2. Use a valid cached thumbnail of the requested size or larger.
3. Stop at Files' own failure record for this version of the file.
4. PNG, JPEG, GIF and WebP: decode in-process with the memory-safe `image`
   codecs and Quick Look's limits (64 MiB file, 16384 pixels a side, 256 MiB
   decoder allocation), applying EXIF orientation.
5. Otherwise, or when built-in decoding fails, the installed thumbnailer for the
   type, in the sandbox below.
6. Store the result with `Thumb::URI`, `Thumb::MTime`, `Thumb::Size`,
   `Thumb::Mimetype` and `Software`; on a real failure (decode error, thumbnailer
   failure, timeout, invalid output) write
   `fail/canopa-files-VERSION/MD5.png`. Unsupported types and cancellation
   record nothing.

Thumbnailers are read once per process from `thumbnailers/*.thumbnailer` in the
XDG data directories (`TryExec` and the program must exist). Installing a new
thumbnailer takes effect at the next start of Files.

For PDFs, when no installed thumbnailer covers them, Files supplies its own
entry: Poppler's `pdftoppm` (the optional dependency Quick Look already uses)
renders the first page, in the same sandbox and bounds. A shell strips the
`.png` that `pdftoppm` adds to its output name. An installed PDF thumbnailer,
such as Papers' or Evince's, takes precedence. Office documents rely on
`gsf-office-thumbnailer`, which shows the thumbnail a document carries.

In List and Columns rows, only pictures and videos show their thumbnail, in a
hairline frame; a page or sheet shrunk to row size reads as a blank tile, so
documents keep their type icon there. Icons view shows every thumbnail.

## Sandbox

External thumbnailers parse untrusted files with C libraries. Each run uses the
same Bubblewrap confinement as the portal's print converters (ADR 0076):

- new user, mount, PID, network, IPC, UTS and cgroup namespaces; nested user
  namespaces disabled; all capabilities dropped; a new session; PID 1 inside, so
  its exit ends every descendant; killed with Files (`--die-with-parent` and a
  parent-death signal);
- a cleared environment: no session bus, display, home or runtime directory;
  `GIO_USE_VFS=local`, `GSETTINGS_BACKEND=memory`;
- read-only `/usr` and the `/bin`, `/lib` links; `/etc` holds only fonts and the
  linker cache; private `/proc` and `/dev`; a 64 MiB `/tmp` as home;
- the input bound read-only from an already opened descriptor at
  `/input/source.EXT` (the original extension, if short and alphanumeric); a
  fresh private folder bound as `/output`;
- only programs under `/usr` can run.

Bounds: 15 s of wall-clock time, then the process group is killed; 30 s of CPU;
2 GiB of address space; 32 MiB per written file; 256 descriptors; no core
dumps. Descriptors other than stdio and the input are close-on-exec before
`bwrap` starts. Output and diagnostics are bounded. The output must be a regular
file (never a planted symlink), at most 32 MiB, decodable within 4096 pixels a
side and 64 MiB, and is always re-encoded: Files never stores the
thumbnailer's bytes or metadata.

Files probes the sandbox once. Without a working `/usr/bin/bwrap` (for example,
unprivileged user namespaces disabled) external thumbnailers are not run at all,
and only built-in images get thumbnails. glycin's own loader sandbox still works
inside, since glycin falls back when it cannot create namespaces.

## Not done

- Per-output scale: the cache size assumes a scale of at most 2.
- Shared thumbnail repositories on removable media (`.sh_thumbnails`).
- Reloading thumbnailers while Files runs.
- Deleting stale cache entries; another desktop's cleaner does that.

Verification: `canopa-assets` `thumbnails` tests (naming, metadata, validity,
cache and failures, thumbnailer parsing); Files `thumbnails` tests (built-in,
cache reuse, policy, failure records, a fake thumbnailer killed at its deadline,
cancellation, validated external output, planted symlinks, and the real
Bubblewrap sandbox: read-only input, no home, no session environment, no
network interfaces, deadline); `thumbnails_ui` tests (visible rows only,
cancellation on navigation, versioned results, the `never` policy); the
`interface_config` round trip. An ignored test renders through the installed
glycin thumbnailer in the real sandbox; it passed on the development machine.
