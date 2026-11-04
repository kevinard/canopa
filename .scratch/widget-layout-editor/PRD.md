# Visual widget layout editor

Status: implemented. Scoped desktop and regression checks passed.

- Add a Widgets page with monitor and scale previews for the Shell bar and
  lockscreen. Make placement, order and membership editable by pointer and
  keyboard.
- Edit independent bars for selected monitors. Preserve module definitions,
  actions, styling and panel settings.
- Move lockscreen widgets between the existing nine regions and configure
  passive widget visibility per monitor. Keep authentication on every display.
- Save through the existing configuration ownership and validation flow.

See [ADR 0041](../../docs/adr/0041-widget-layout-editor.md).

## Validation

- The final nested Weston/Niri run selected and moved a bar widget in Settings,
  saved it and confirmed the running Shell reloaded it. It saved a hidden widget
  for the preview monitor and verified that Canopa Lock rendered different
  content for that monitor. Screenshots and logs are in the ignored
  `.scratch/artifacts/widget-layout-editor/` directory.
- The Settings desktop suite passed all 12 checks with the optional Stasis
  backend. The independent lockscreen suite passed 25 checks with test-owned PAM
  and logind. Physical monitor and hardware authentication tests were outside this
  run.
- Settings and session consumer tests, focused Clippy, workspace Clippy, the
  installer and unit graph, Settings portal, configuration and vendor tests,
  and the offline documentation bundle checks passed.
- The workspace command was interrupted after the installer and check-script
  stages. Its remaining tooling checks passed separately. The workspace test
  build stopped at the Files video playback test
  `video_uses_the_opened_file_for_paused_frames_seeks_rewind_and_owner_drop`.
  It failed again in the workspace-built executable and passed in the focused
  Files build.
