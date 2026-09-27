# Focused Window Rotation Prototype

Question: can an iced-native rendering path keep focused-window title text more legible than the current generated-SVG-then-rotated path?

Run:

```bash
rtk cargo run --manifest-path experiments/focused-window-rotation/Cargo.toml
```

Variants:

- A: generated SVG snapshot, then `Svg::rotation`
- B: iced Canvas glyph outlines rotated in the frame
- C: actual iced widget subtree rotated through a patched renderer transformation path

Patch finding: upstream iced 0.14 blanks C because text rendering derives glyph scale from `Transformation::scale_factor()`, which reads the matrix x-axis x component. At an exact 90-degree rotation that value is 0, so transformed text is rendered at zero scale. The local `vendor/iced_core` patch changes scale extraction and transformed bounds to be rotation-aware, then C can test how much more is needed.

Second patch finding: after C becomes visible, text still remains upright because the `iced_wgpu` text path passes only `left`, `top`, `scale`, and bounds into `cryoglyph::TextArea`. There is no rotation channel in `cryoglyph`, and its shader expands cached glyphs as axis-aligned quads. The prototype now vendors and patches `iced_wgpu` plus `cryoglyph` to pass a rotation angle per text area and rotate glyph quads in the text shader.

This makes C a three-crate renderer patch, not a small `iced_core` patch. It is useful as a proof of concept, but productionizing it would still need a proper public transform API and robust clipping for rotated text.
