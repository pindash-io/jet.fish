# jetfish_protocol

Screen-agnostic terminal protocol primitives shared by the jet.fish terminal
engines: the cell-width model, packed SGR attributes and color spans, grid
geometry, tab stops, the dynamic palette/theme, and the byte/text helpers.

It is a facade over the parser and width-model crates — screens depend on
`jetfish_protocol` alone and never re-declare which parser/width/palette
backend they use.

## Modules

| module | purpose |
| --- | --- |
| [`attrs`](src/attrs.rs) | packed SGR attributes (`Attrs`) + flat `Style` projection |
| [`span`](src/span.rs) | sorted SGR color spans (`Span`/`Spans`) |
| [`width`](src/width.rs) | byte ↔ cell width walkers (floor/ceil/end) |
| [`geometry`](src/geometry.rs) | viewport / scroll-region / margin math |
| [`tabstops`](src/tabstops.rs) | horizontal tab stops |
| [`text`](src/text.rs) | char boundaries, line ranges, word bounds |
| [`uri`](src/uri.rs) | percent-decoding and OSC 7 working directories |
| [`palette`](src/palette) | dynamic palette, base16/color256, theme |
| [`keyboard`](src/keyboard.rs) | mouse-report encoding, alt-screen flags |
| [`hash`](src/hash.rs) | deterministic foldhash maps |
| [`blink`](src/blink.rs) | text/cursor blink phases |

## License

Licensed under either of [Apache-2.0](../../LICENSE-APACHE) or
[MIT](../../LICENSE-MIT) at your option.
