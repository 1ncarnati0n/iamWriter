# Local dependency patches

Cargo uses these sources through `[patch.crates-io]`; versions are unchanged.
Sources copied from the installed crates.io packages, with original licenses retained.
Do not edit the Cargo registry cache. Reapply or remove these small changes when updating dependencies.

## iced_graphics 0.14.0

Changed `src/text.rs::to_attributes` only. `Font::Style::Oblique` is mapped to the normal face plus cosmic-text's `FAKE_ITALIC` cache flag. This keeps the chosen family and slants Hangul fallback glyphs too. Native `Italic` requests remain unchanged. The app uses `Oblique` for Markdown italics and combined bold italics.

Regression coverage in `src/tests.rs` checks glyph flags, unchanged family, real bold font weights, and differing regular/slanted raster output.

## winit 0.30.13

Changed `src/platform_impl/macos/view.rs::key_down` only. On the first key after switching to a Korean input source (or initial IME activation), if AppKit produces a lone compatibility Jamo without starting marked text or committing text, retry `interpretKeyEvents` once after 20ms. The original raw key is emitted if the retry also fails. Existing committed/preedit text is not replayed. No custom Jamo composer, dropped key, background timer, or unbounded retry.

This is a local, bounded mitigation, not the unmerged upstream PR implementation. It briefly blocks the main thread only on that failure path. Actual input-source switching, fast typing, and IME candidate interaction remain unverified.

References:
- https://github.com/rust-windowing/winit/issues/3095
- https://github.com/rust-windowing/winit/pull/4693

Removal condition: upgrade to an upstream fix verified on macOS Korean 2-Set input, then remove this retry. Do not blindly replace the current backend with an unmerged master-branch PR.
