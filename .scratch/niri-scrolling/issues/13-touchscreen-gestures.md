# Touchscreen gestures

Status: implemented. Fork PR kevinard/niri#18, pinned (now `b4fc5ea7`); not tried on
touch hardware.

Branch `canopa/touch-gestures` (`659d887f` on `canopa/remote-input-touch`, PR #18):
"input: Add touchscreen swipe gestures".

- Recognised: 3 or 4 fingers landing within 300 ms of the first, whose centre
  then moves 16 logical px (the touchpad threshold). 3 fingers horizontal
  scrolls the view, 3 fingers vertical switches workspace, 4 fingers vertical
  opens (up) or closes (down) the overview; content follows the fingers.
- Never recognised: 1–2 fingers (unchanged path), late fingers (pinches),
  fingers lifted before recognition, 5+ fingers, 4-finger horizontal.
- Clients get touches as usual until recognition; that frame sends
  `wl_touch.cancel` and later touches are withheld until every finger lifts.
- Not started during other grabs (interactive move, drag and drop), the
  screenshot UI, the window switcher, the lock screen, or while any
  `RemoteInput1` touch lease is open, because the cancel ends all of a client's
  touches, remote ones included (ADR 0117). Remote touch devices never reach
  the recogniser; slot bookkeeping still runs for every physical touch-up.
- `gestures { touchscreen-swipe { off; } }` turns it off; on by default.

Risks: Smithay sends the cancel only with pending events in the frame
(recognition always follows forwarded motion, so it should hold); the cancel
reaches every client holding touches; in the overview a first finger that moves
16 px before the centre does wins the existing one-finger gesture.

## Validation

- Fork, own profile: `cargo test --locked -p niri` 398, niri-config 36 + 1,
  niri-ipc 2 + 1 pass. New: 10 recogniser unit tests and 7 integration tests
  (touch events added to the test backend and `wl_touch` to the test client),
  including a regression test that failed with the overview-touch fix removed.
  The 13 remote-touch contact and 3 wire tests still pass.
- No touchscreen, live or nested session.

## Comments
