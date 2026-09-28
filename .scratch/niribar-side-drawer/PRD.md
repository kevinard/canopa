# Consistent vertical side drawer

Status: ready-for-agent

Implementation: complete, validated, and installed in the live session.

The user selected the side-drawer direction from the vertical interaction
prototype and clarified that it should show only the clicked module. Bar icons
are the navigation; there are no shared-system section tabs.

## Accepted behavior

- Left/right bars can present module panels, provider panels, named menus and
  dmenu content in one consistent drawer position and width.
- The default drawer width is 360 logical pixels and height fills the usable
  vertical edge. Exact configured dimensions and maximum dimensions still apply.
  A height-capped drawer remains at the start of the usable edge.
- The active content has a readable heading and explicit Close control. Provider
  titles are preserved without adding a duplicate title above them.
- Single module detail content sits directly in the drawer body. Menu rows use
  restrained fills, with a visible focused row.
- Existing module repeat-click, replacement, keyboard and close behavior remains
  owned by Foreground. No exclusive space is reserved for the drawer.
- `panel { presentation "drawer"; }` enables this on vertical bars; the shipped
  default selects it. Omission or `"auto"` preserves existing configuration behavior.
  Horizontal bars retain automatic presentation. Tooltips and SNI context menus
  retain their existing popup behavior.

## Boundaries

This is a presentation change. Provider protocol, lifecycle, notification policy,
module actions, and module detail capabilities remain owned by their existing
components. The prototype's simulated volume slider is not part of this work.
The shipped configuration now binds primary activation of clock, volume, battery
and focused-window to their existing module detail panels. Custom configurations
keep their own bindings.

## Validation

Check strict configuration parsing, left/right placement across content types,
module anchors near both ends of the bar, explicit dimensions and caps, unchanged
horizontal sizing, provider heading/close controls, and desktop open/close behavior.
Run the workspace's normal `bash scripts/check.sh` validation.

Design evidence: `.scratch/niribar/vertical-ui-prototype/` (original alternatives),
archived on local branch `prototype/niribar-vertical-ui` at `fa946b9`.

Native desktop acceptance passed with left/dark at 100% and right/light at 125%:
stable full-height calendar/menu placement, module replacement, repeat-click
dismissal, Escape, and the Close button. Screenshots and results are under
`evidence/`. The fixture only connects to its private Weston/Niri/D-Bus session.

To repeat after building `target/debug/niribar`:

```sh
python3 .scratch/niribar-side-drawer/verify-desktop.py /tmp/niribar-drawer-acceptance
```

The output directory must be new. Optional final argument `left` or `right` runs
only that case. The fixture needs the same tools as the repository's desktop
scripts: Weston, Niri, wtype, a C compiler, Wayland development tools, and Pillow.

## Completed checks

- Format and diff checks, workspace Clippy, dependency/protocol/provenance checks,
  theme contrast checks, updater/session tests, configuration tests, Settings
  portal tests, and vendor tests passed.
- All non-Niribar workspace test groups passed. Niribar's final serial run passed
  all targets, including all 1,175 application tests and its provider/integration
  tests. Two pre-existing Wi-Fi tests hit their three-second deadlines in a
  parallel rerun and passed serially. The new embedded-default test was corrected
  to parse the shipped KDL rather than the empty `Config::default()` constructor.
- `cargo build --locked --offline -p canopa-cli -p niribar --bins` passed.
- Native desktop acceptance passed as described above; screenshots were inspected.
- Live configuration follow-up: an existing `height 720` overrode automatic drawer
  height. Removing that setting restored full-height sizing. The corrected config
  passed validation and the restarted service confirmed it loaded the new source.
  All six drawer tests passed again; the README now calls out legacy height limits.

The normal `scripts/check.sh` run was interrupted during compilation; its remaining
groups were completed individually. Detailed logs for this session are in
`/tmp/canopa-side-drawer-{check,tests,niribar-serial,final-clippy,script-checks,build}.log`.
