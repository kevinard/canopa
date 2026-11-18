# 0155. Generate the compositor titlebar style from the shared appearance

Status: accepted

## Context

ADR 0153 keeps SSD titlebars in the Niri fork, which reads their colors and font
from its own configuration. The shipped defaults matched only the dark appearance,
so a light, high-contrast or customized appearance left titlebars dark. The
compositor cannot read `config.kdl`, and the Shell, which reloads `appearance`
live, does not own compositor files.

## Decision

Settings owns the translation. It generates `~/.config/canopa/niri-appearance.kdl`
from `appearance` in `config.kdl`: the surface, background, text, muted and danger
roles become the titlebar colors, and the typography families at the body size
times `text-scale` become a Pango font description. It writes the file at every
session start and after Personalization saves, validating with Niri before
replacing it, and reloads Niri when it changed.

`niri.kdl` includes the generated file before `niri-custom.kdl`. Session start adds
that line to sessions created earlier, immediately before their
`include "niri-custom.kdl"`; a configuration without that line is left alone.

## Consequences

Titlebars match the appearance chosen in Settings without a compositor protocol or
a running Shell. Options in `niri-custom.kdl`, including window rules, still
override the generated style. `config.kdl` edits made outside Settings apply at the
next login rather than live. The palette mapping lives in one Settings module and
must follow new appearance roles.
