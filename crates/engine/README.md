# jetfish_engine

<!-- cargo-rdme start -->

`jetfish_engine` — the rendering contract between the screens and the
window: the [`TextEngine`](https://docs.rs/jetfish_engine/latest/jetfish_engine/engine/trait.TextEngine.html) trait (five facets), the shared glyph/image
types, the [`ScreenBackend`](https://docs.rs/jetfish_engine/latest/jetfish_engine/backend/trait.ScreenBackend.html) factory, and the shared [`tuning`](https://docs.rs/jetfish_engine/latest/jetfish_engine/tuning/).

A leaf crate: depends only on `jetfish_text` and `jetfish_protocol`.

<!-- cargo-rdme end -->

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
