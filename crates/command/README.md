# jetfish_command

<!-- cargo-rdme start -->

`jetfish_command` — the decoded command model plus the apply contract. The
byte parser decodes into a [`Command`](https://docs.rs/jetfish_command/latest/jetfish_command/command/enum.Command.html), a screen applies it via
[`Apply::apply`](https://docs.rs/jetfish_command/latest/jetfish_command/commands/trait.Apply.html#tymethod.apply); the crate owns the parsers, codec, [`Commands`](https://docs.rs/jetfish_command/latest/jetfish_command/commands/struct.Commands.html) buffer,
and [`Recording`](https://docs.rs/vtty_core/latest/vtty_core/event/struct.Recording.html) log.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
