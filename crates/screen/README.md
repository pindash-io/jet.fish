# jetfish_screen

<!-- cargo-rdme start -->

`jetfish_screen` — the shared screen shell: the main/alt dual-buffer
[`Screen`](https://docs.rs/jetfish_screen/latest/jetfish_screen/screen/struct.Screen.html) and the [`ScreenShared`](https://docs.rs/jetfish_screen/latest/jetfish_screen/shared/struct.ScreenShared.html) context both screen models embed.

The cell screen itself (`CellScreen`) lives in `jetfish_cell`.

- `screen.rs` — generic [`Screen`](https://docs.rs/jetfish_screen/latest/jetfish_screen/screen/struct.Screen.html) shell.
- `shared.rs` — [`ScreenShared`](https://docs.rs/jetfish_screen/latest/jetfish_screen/shared/struct.ScreenShared.html) (grid, palette, OSC 52 policy, …).
- `buffer.rs` — per-buffer `BufferState`.
- `input_state.rs` — mouse and Kitty-keyboard input state.

<!-- cargo-rdme end -->

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
