# Launcher unit conversions — 2026-11-03

Added conversions to the existing calculator provider and Panel Menu. Queries use
a numerical amount and compatible units separated by `to`, `in`, `->`, or `→`.
Arithmetic quantities, common unit names/symbols, powers, and simple ratios are
normalized into trusted Qalculate expressions. Conversion factors and absolute
temperature offsets remain owned by Qalculate. Enter copies the value and unit.

The host validates source/target dimensions before evaluating. Converted output
must contain a complete finite number and the requested target unit after alias
normalization. Conversions load built-in units with currencies, datasets,
functions, variables, and personal state disabled. Arithmetic retains its existing
definitions-disabled mode. Both keep the existing time/output/cancellation bounds.

## Validation

- `bash scripts/cargo.sh test --locked -p canopa-shell --bin canopa-shell launcher --quiet`:
  **18 passed**. Includes existing calculator/provider/session regressions, 23 real
  conversion fixtures, and Qalculate output compatibility for all 62 registered units.
- The fixtures cover imperial/metric length and mass, absolute Celsius/Fahrenheit/
  Kelvin, negative temperatures, decimal commas, duration, pressure, energy, power,
  angles, decimal/binary information, bits/bytes, speed, area, volume, and numerical
  arithmetic before the source unit. Invalid dimensions, partial queries, arbitrary
  command tokens, malformed/non-finite output, and incorrect output units are rejected.
- Shell binary/test Clippy with warnings denied, formatting, and whitespace checks pass.
- Debug binary builds through the guarded Cargo wrapper.
- The documentation bundle stages **55 files and 80 resolving offline links**.
- `scripts/verify-shell-launcher.py --scale 1.25` passes **15 native journeys** in
  private Weston/Niri, including new length, temperature, binary data, speed, and
  incompatible-unit recovery journeys. Clipboard values remain available after
  the panel dismisses. Existing application/file/extension journeys also pass.

Native logs, configuration, screenshots, and result JSON are under
`.scratch/artifacts/unified-launcher/unit-conversions-native-125/`. The temperature
screenshot was inspected for the input, degree symbol, value, and copy hint.

The first unit-definition compatibility test exposed Qalculate's `cal_th`,
`kcal_th`, `W*h`, and `kW*h` output aliases; normalization now handles those explicitly.
CLI probes also established the mixed-unit target modifier and absolute temperature
mode before production integration. No installed configuration or live desktop was
changed. A full workspace or full shell-suite rerun was not part of this narrow
calculator follow-up; the earlier baseline evidence remains in [VALIDATION.md](VALIDATION.md).

See the [user guide](../../docs/shell/launcher.md#unit-conversions) for syntax and
the supported unit families.
