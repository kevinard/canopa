# Launcher character catalogs

`emoji.tsv` keeps the fully qualified sequences and names from
[Unicode Emoji 17.0 test data](https://www.unicode.org/Public/17.0.0/emoji/emoji-test.txt)
(© 2025 Unicode, Inc.). English search keywords come from the
[CLDR 48 English annotations](https://raw.githubusercontent.com/unicode-org/cldr/release-48/common/annotations/en.xml)
(© 1991–2025 Unicode, Inc.).

`symbols.tsv` uses the same annotations for single-character symbols and adds
common math, Greek, and keyboard characters. Both catalogs use
[Unicode License v3](LICENSE-UNICODE).

To regenerate, download the two source files above and run from the repository
root with Python 3.

```sh
python3 crates/canopa-shell/data/characters/generate.py emoji-test.txt en.xml
```

| Source | SHA-256 |
| --- | --- |
| `emoji-test.txt` | `1d8a944f88d7952f7ef7c5167fef3c67995bcae24543949710231b03a201acda` |
| `en.xml` | `8511aadd046fdba2f0ffe590266ced8bbf48175ad139b2675d85d7141057b235` |
