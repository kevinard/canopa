# 0142 The 12/24-hour clock lives in the clock format strings

Status: accepted, amended by [ADR 0145](0145-fluent-translations-and-icu-formatting.md). Builds on [ADR 0136](0136-settings-pages-patterns-and-activation.md).

The Date & time page offers a 12- or 24-hour clock for Canopa. Two clocks
show the time: each Shell clock module, with its Chrono `time-format`
(default `%H:%M`), and the lock screen, with `lockscreen { clock-format }` in
`config.kdl` (default `%H:%M`). People already write their own patterns in
both, for example `%H:%M:%S` or `%a %H:%M`.

## Options considered

- **A new shared key** (for example `appearance { clock "12h" }`) that the
  Shell and the lock screen read and combine with their formats. It would
  need a precedence rule against the existing strings, changes in two
  readers, and a migration, and a custom pattern would silently stop meaning
  what it says. Rejected.
- **The locale's `LC_TIME`.** Canopa's formats are explicit Chrono patterns,
  not locale-driven; following `LC_TIME` would change every clock's look, not
  only the hour cycle. Rejected; Region & language may revisit this.
- **The existing format strings.** Settings reads each clock's pattern and,
  when it is one of the usual 24- or 12-hour patterns, switches it to the
  other form, keeping seconds. Chosen.

## Decision

There is no separate preference. `crates/canopa-settings/src/clock_format.rs`
recognizes `%H:%M`, `%R`, `%k:%M` (24-hour) and `%-I:%M %p`, `%I:%M %p`,
`%l:%M %p` (12-hour), with their seconds forms, and writes `%H:%M[:%S]` or
`%-I:%M[:%S] %p`. The page shows 24- or 12-hour when every clock agrees, and
names each clock with another pattern: that pattern is the person's own and
is never rewritten. An unset 24-hour default stays unset.

Clock modules are the built-in `module "clock"` and any `module` with
`type "clock"` under `shell { modules }`. When `config.kdl` has no `shell`
block, the shipped Shell defaults are written out first, as the Widgets page
does. The file is validated and saved like other `config.kdl` changes; the
Shell reloads it live and the lock screen reads it when it locks. Widgets and
Personalization keep their unsaved drafts under the shared-document rule;
Personalization's draft also takes the new lock-screen format.

## Consequences

- No reader changes, and the strings stay the single source of truth.
- A clock with a custom pattern keeps it; the page says so.
- Other places that show times (Notify's banners, the Date & time page) use
  their own fixed formats.

## Amended by ADR 0145 (2026-11-11)

With [ADR 0145](0145-fluent-translations-and-icu-formatting.md), a clock
without a pattern follows the language's formats: the Shell's clock modules
no longer ship `time-format "%H:%M"`, and an absent or empty lock-screen
`clock-format` uses the locale's time style. "Unset" therefore no longer
means 24-hour.

The Date & time page offers **Automatic**, **24-hour** and **12-hour**:

- **Automatic** is every clock without a pattern. Choosing it removes the
  patterns `clock_format.rs` recognizes (`%H:%M`, `%R`, `%k:%M`,
  `%-I:%M %p`, `%I:%M %p`, `%l:%M %p` and their seconds forms) from the
  clock modules and the lock screen. The page says which cycle that gives
  now, from `canopa_i18n::format::uses_24_hour_clock()` for the selected
  formats.
- **24-hour** and **12-hour** write the explicit pattern as before, now also
  for unset clocks (an unset clock may be 12-hour in English).
- A custom pattern is still never rewritten or removed, and the page still
  names it.

The page shows a choice when every clock agrees. The shipped Shell defaults
are written out only when a clock in them changes, so this works whether or
not they set `time-format`. Personalization leaves an empty clock or date
format out of `config.kdl` instead of writing an empty string.

Other places that show times (Notify, the Date & time page's clock) use the
locale's formats.
