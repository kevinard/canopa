# Translations and locale formatting

Canopa's user-visible text is in [Project Fluent](https://projectfluent.org/)
files, one per crate and language, embedded in the binaries at build time.
Dates, times, numbers and sizes are formatted with ICU4X for the person's
formats locale. The decision record is
[foundations](architecture/foundations.md).

English is the source language and the fallback for every message. French is
the first translation; it is machine-assisted and needs review (see
[Reviewing the French translation](#reviewing-the-french-translation)).

## Which language a process uses

`canopa_i18n::init()` (called at the start of every application's `main`)
negotiates, first match wins:

1. `CANOPA_LANGUAGE` (`fr`, `fr:en`), for development and tests;
2. `LC_ALL`;
3. the person's `~/.config/locale.conf` (`LC_MESSAGES`, then `LANG`, with its
   `LANGUAGE` list), which the Settings Region & language page writes;
4. the inherited `LC_MESSAGES` and `LANG`, with the inherited `LANGUAGE`.

`LANGUAGE` is ignored with the `C` or `POSIX` locale. The result is matched
against the shipped languages (`canopa_i18n::SHIPPED`) and always ends with
English. Formats follow the same order with `LC_TIME` (dates and times) and
`LC_NUMERIC` (numbers and sizes).

Because `locale.conf` comes before the inherited session variables, a Canopa
application restarted after a change in Settings uses the new language at
once; other applications follow at the next login. Settings itself switches
live.

Until `init()` runs, a process uses English and `en` formats. Unit tests and
in-process integration tests therefore run in English whatever the machine's
locale; tests that start a Canopa binary set `CANOPA_LANGUAGE=en`.

`CANOPA_PSEUDO_LOCALE=1` selects the pseudo-locale: English with accented
letters (`Šéţţîñĝš`), about 40% longer, framed by `[` `·]` and written right
to left, so clipped labels, untranslated literals and missing mirroring are
visible. `canopa_i18n::pseudo::strip` turns it back into English for tests.

## How to add a string

1. Add the message to the crate's English file,
   `crates/<crate>/i18n/en/<crate_name>.ftl` (`canopa_files.ftl` for
   `canopa-files`), next to related messages:

   ```ftl
   # Shown when a copy finishes. $count is the number of files.
   files-copy-finished = { $count ->
       [one] Copied { $count } file
      *[other] Copied { $count } files
   }
   ```

   IDs are kebab-case, start with the crate's area (`files-`, `settings-`,
   `shell-`, `notify-`, `portal-`, `lock-`), and describe the meaning, not the
   English words. Add a comment when the context is not obvious (where it is
   shown, what each variable holds, length limits).
2. Use it with the crate's `fl!`, whose ID and arguments are checked at
   compile time against the English file:

   ```rust
   text(fl!("files-copy-finished", count = done))
   ```

   Pass numbers as numbers (for plurals and localized digits) and names as
   strings. Years, ports and other numbers that must not get digit grouping
   are passed as strings.
3. Add the French message to `i18n/fr/<crate_name>.ftl` (or leave it out:
   English is used until it is translated, but each crate's
   `check_catalog` test then fails, so every shipped language stays complete).
4. Run the crate's tests and `python3 scripts/check-i18n.py --crate <crate>`.

Rules:

- One message per complete sentence or label. Never build a sentence by
  joining translated fragments; use placeables (`{ $name }`) and selectors.
- Message IDs, accessibility keys and focus IDs are separate. Keys and IDs
  (`settings-page-display`, `files-toolbar-back`) never come from translated
  text, so focus and screen-reader identity survive a language change
  ([foundations](architecture/foundations.md)). Accessible names are the
  translated labels.
- Code must not inspect translated text (`label.starts_with("Account: ")`);
  carry the meaning in data and translate only for display.
- Log lines, panics and internal errors stay English. Errors shown to people
  (dialogs, notifications, status lines) are translated.
- Dates, times, numbers and sizes go through `canopa_i18n::format`
  (`date`, `time`, `date_time`, `weekday`, `month_year`, `first_weekday`,
  `integer`, `decimal`, `percent`, `size`, `decimal_size`, `duration`,
  `relative`), never `format!("{:.1}")` or a fixed Chrono pattern.
- Clocks keep a person's explicit Chrono pattern. Without one they use the
  locale's time and date styles; Settings' 24- and 12-hour choices write a
  pattern and its Automatic choice removes the patterns it recognizes
  ([foundations](architecture/foundations.md)).
- Layouts use `canopa_ui::direction` for rows with leading and trailing
  content, alignment, padding, chevrons, back arrows and directional icons.

### Setting up a new crate

```toml
# crates/<crate>/i18n.toml
fallback_language = "en"

[fluent]
assets_dir = "i18n"
```

```rust
// crates/<crate>/src/i18n.rs
canopa_i18n::localization!();
```

Declare `#[macro_use] mod i18n;` at the top of `lib.rs` (and of `main.rs` when
the binary has its own modules) so `fl!` is available in later modules, add
`canopa-i18n.workspace = true` to `[dependencies]`, call `canopa_i18n::init()`
first thing in `main`, and add a catalog test:

```rust
#[test]
fn catalog_is_complete() {
    canopa_i18n::check_catalog(&crate::i18n::LOADER).unwrap();
}
```

The check fails when a shipped language misses a file or a message, has an
extra one, uses other variables than English, lacks the review header, or
(for French) breaks the typographic rules below.

## The literal lint

`scripts/check-i18n.py` (run by `scripts/check.sh --workspace`) rejects
word-like literals passed to `text`, `label`, `button`, `tooltip`,
`text_input`, notification summaries and bodies and similar calls and fields
in the UI crates, outside test modules. It is a heuristic: a literal that is
not shown to people (a protocol string, a brand name, a debug tool) goes in
`scripts/i18n-allowlist.txt` with a reason.

- `python3 scripts/check-i18n.py --crate canopa-files` checks one crate.
- `--report` prints, per crate, the `fl!` lookups, the lint hits and the
  capitalized multi-word literals left (a noisy upper bound).
- `--prose` lists those literals, to find what the call heuristic misses.
- `python3 scripts/test-check-i18n.py` runs the lint's own tests.

## How to add a language

1. Add its tag to `SHIPPED` in `crates/canopa-i18n/src/negotiate.rs`; if it is
   written right to left and not yet listed, add it to `is_rtl_language`.
2. For every crate with an `i18n/en/` directory, copy the English file to
   `i18n/<tag>/<crate_name>.ftl`, translate every message, and start the file
   with a header that says whether it is reviewed
   (`machine-assisted, needs review` until a fluent speaker has read it).
3. Add `Name[<tag>]`, `GenericName[<tag>]`, `Comment[<tag>]` and
   `Keywords[<tag>]` to the desktop entries in `packaging/applications/` and
   the session entries in `packaging/wayland-sessions/` and `packaging/system/`.
4. Run the workspace tests: every crate's `check_catalog` test covers the new
   files. For a right-to-left language, also look at the main windows with
   `CANOPA_LANGUAGE=<tag>`, and use `CANOPA_PSEUDO_LOCALE=1` to compare.

ICU4X's compiled CLDR data already covers the formats of every locale.

## Reviewing the French translation

Each French file starts with `machine-assisted, needs review`. To review:

1. Read `crates/*/i18n/fr/*.ftl` beside the English files; comments in the
   English files give the context of each message.
2. Run an application in French with `CANOPA_LANGUAGE=fr` (for example
   `CANOPA_LANGUAGE=fr cargo run -p canopa-settings`), or set French on the
   Region & language page.
3. Fix messages in place. Keep the IDs and the `{ $variables }`; the catalog
   tests reject a translation whose variables differ from English.
4. Typography: a no-break space (U+00A0) before `:` `;` `!` `?` and `»` and
   after `«`, the apostrophe `’`, and « guillemets » for quotations. The
   catalog tests reject plain spaces before those marks and straight quotes.
5. When a file has been read end to end, change its header to
   `reviewed by <name>, <date>`.
