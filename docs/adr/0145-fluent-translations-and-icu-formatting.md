# 0145 Fluent translations, ICU4X formatting and right-to-left layouts

Status: accepted. Builds on [ADR 0003](0003-shared-appearance.md),
[ADR 0142](0142-clock-hour-cycle-in-format-strings.md) and
[ADR 0144](0144-shared-accesskit-layer.md). Implements the translation part of
PRD W5 (`.scratch/distro-readiness/PRD.md`).

Before this, every Canopa component showed hard-coded English, clocks and
timestamps used fixed Chrono patterns (`%H:%M`, `%Y-%m-%d · %H:%M`), sizes
were `format!("{:.1} MiB")`, the Shell calendar printed English weekdays from
Monday, and no layout mirrored for right-to-left languages.

## Options considered

- **gettext** (`gettext-rs`, `.po` files). Mature tooling, but message IDs are
  English source strings, plurals are limited to one numeric argument, and IDs
  are not checked at compile time.
- **Project Fluent through `i18n-embed` and `i18n-embed-fl`.** Rust-native,
  used by COSMIC, supported by Weblate; `fl!` checks message IDs and argument
  names against the English file at compile time; selectors handle plurals and
  grammatical variants. Chosen.
- **Formatting with Chrono's `unstable-locales`** (glibc-style patterns per
  locale) or **through glibc** (`strftime` with `LC_TIME`). The first keeps
  fixed patterns and has no number or week data; the second depends on the
  installed locales and on process-global `setlocale`. ICU4X (CLDR) gives
  date, time, number and week data for every locale in pure Rust. Chosen.

## Decision

- **One shared crate, `canopa-i18n`**, Iced-free so headless profiles and
  services can use it (`scripts/check-core-dependencies.sh` and
  `scripts/check-provider-dependencies.sh` still pass). It wraps
  `i18n-embed`'s `FluentLanguageLoader`:
  - each crate keeps `i18n.toml` and `i18n/<language>/<crate_name>.ftl`,
    embedded with `rust-embed` (`debug-embed`, so debug builds and tests never
    read files at run time) and declared with `canopa_i18n::localization!()`,
    which gives the crate a `LOADER` and an `fl!`;
  - a process-wide selection (message languages, `LC_TIME` locale,
    `LC_NUMERIC` locale, pseudo-locale) with a generation counter: every
    crate's loader reloads on its next lookup after `canopa_i18n::select`, so
    a window shows a new language at its next redraw;
  - negotiation from `CANOPA_LANGUAGE`, `LC_ALL`, the person's
    `~/.config/locale.conf`, then the inherited `LC_MESSAGES`/`LANG` with
    `LANGUAGE`, matched against the shipped languages with English last. The
    person's file comes before the inherited variables so that a Canopa
    application restarted after a Settings change follows it before the next
    login;
  - English until `init()`, so tests are deterministic on any machine;
  - Fluent numbers formatted with ICU4X for `LC_NUMERIC` (`set_formatter`),
    and bidi isolation marks only for right-to-left languages;
  - `check_catalog`, used by every crate's tests: each shipped language has a
    file with exactly the English IDs and variables, a review-status header,
    and (French) no-break spaces before `: ; ! ?` and inside « ».
- **Formatting** (`canopa_i18n::format`) uses ICU4X 2 with compiled CLDR
  data in the Gregorian calendar: dates in four styles, times with an
  optional hour-cycle override, weekday and month names, the region's first
  weekday, whether it uses a 24-hour clock, integers and decimals. Sizes
  (binary for files, decimal for disks), durations and relative times are
  Fluent messages around ICU numbers, with plural rules (`ko`/`Mo` and
  `Kio`/`Mio` in French).
- **Pseudo-locale.** `CANOPA_PSEUDO_LOCALE=1` accents the literal text of
  every message (placeables stay readable), lengthens it by about 40%, frames
  it and embeds it right to left; its direction is RTL, so it also exercises
  mirroring.
- **Right to left.** `canopa_ui::direction` gives rows in reading order,
  leading/trailing alignment and padding, mirrored chevrons, back arrows and
  directional icon names. cosmic-text already shapes bidi text, in Iced and
  in `canopa-session-ui`'s CPU renderer.
- **Clocks.** An explicit Chrono pattern is kept as the person's own. Without
  one (an absent `time-format`, or an empty `lockscreen { clock-format }`),
  the Shell clock, its calendar and the lock screen use the locale's time and
  date styles, and the calendar starts weeks on the region's first weekday.
  The Date & time page offers Automatic, 24-hour and 12-hour: Automatic
  removes the patterns it recognizes, the other two write them (ADR 0142,
  amended).
- **Shipped Shell labels.** `default.kdl` names its built-in labels with
  `label-id`, `tooltip-id`, `title-id` and `prompt-id` message IDs, so the
  defaults are translated while a person's own `label "…"` stays literal.
- **Lint.** `scripts/check-i18n.py` rejects word-like literals passed to
  text, label, button, tooltip and placeholder calls and to label, title,
  summary and body fields in UI crates, with `scripts/i18n-allowlist.txt` for
  justified exceptions; `scripts/check.sh --workspace` runs it and its tests.
- **Desktop entries** carry `[fr]` values for `Name`, `GenericName`,
  `Comment` and `Keywords`.

How to add a string or a language: [docs/translations.md](../translations.md).

## Consequences

- Message IDs, accessibility keys and focus IDs are independent: translating
  never changes focus or AT-SPI identity (ADR 0144), and accessible names are
  the translated labels.
- Code that compared displayed text (label prefixes, English status words)
  carries the meaning in data instead.
- Settings applies a language change to itself at once and tells the person
  which applications need a restart; other applications read the new
  `locale.conf` when they start.
- Binaries grow by the CLDR data they use (date, time, decimal and plural
  data for all locales, a few MB). Generating baked data for the shipped
  locales only (`icu4x-datagen`) would shrink it but would also lose correct
  formats for regions people use with an English or French interface
  (`en-GB`, `fr-CA`); revisit if size matters.
- Only the Gregorian calendar is used for display.
- The French translation is machine-assisted and marked for review in each
  file's header; the maintainer reviews it as described in
  `docs/translations.md`.
- Input methods (fcitx5), CJK setup and an RTL translation remain open under
  PRD W5.
