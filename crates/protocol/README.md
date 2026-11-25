# jetfish_protocol

<!-- cargo-rdme start -->

`jetfish_protocol` — the screen-agnostic protocol primitives: the cell-width
model, SGR spans/attributes, text helpers, tab stops, grid geometry, and
themes. The decoded command model (`Command`/`Apply`) lives in
`jetfish_command`; screen models (cell grid, string+span) implement both in
their own crates.

| module | purpose |
| --- | --- |
| [`attrs`](https://docs.rs/jetfish_protocol/latest/jetfish_protocol/attrs/) | packed SGR attributes + flat `Style` projection |
| [`span`](https://docs.rs/jetfish_protocol/latest/jetfish_protocol/span/) | sorted SGR color spans |
| [`width`](https://docs.rs/jetfish_protocol/latest/jetfish_protocol/width/) | byte ↔ cell width walkers |
| [`geometry`](https://docs.rs/jetfish_protocol/latest/jetfish_protocol/geometry/) | viewport / scroll-region / margin math |
| [`tabstops`](https://docs.rs/jetfish_protocol/latest/jetfish_protocol/tabstops/) | horizontal tab stops |
| [`text`](https://docs.rs/jetfish_protocol/latest/jetfish_protocol/text/) | char boundaries, line ranges, word bounds |
| [`uri`](https://docs.rs/jetfish_protocol/latest/jetfish_protocol/uri/) | percent-decoding and OSC 7 working directories |
| [`palette`](https://docs.rs/jetfish_protocol/latest/jetfish_protocol/palette/) | dynamic palette, base16/color256, theme |
| [`keyboard`](https://docs.rs/jetfish_protocol/latest/jetfish_protocol/keyboard/) | mouse-report encoding, alt-screen flags |
| [`hash`](https://docs.rs/jetfish_protocol/latest/jetfish_protocol/hash/) | deterministic foldhash maps |
| [`blink`](https://docs.rs/jetfish_protocol/latest/jetfish_protocol/blink/) | text/cursor blink phases |

<!-- cargo-rdme end -->

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
