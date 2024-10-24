# Hexciv

[Civ VI] inspired game, written in [Rust] using the [Bevy] game engine.

[Bevy]: https://bevyengine.org/
[Civ VI]: https://civilization.fandom.com/wiki/Civilization_VI
[Rust]: https://www.rust-lang.org/

## Setup

### Setup matchbox_server

```
cargo install matchbox_server
```

### Setup wasm-server-runner

Install and setup [wasm-server-runner].

[wasm-server-runner]: https://github.com/jakobhellermann/wasm-server-runner

## Run

### Run matchbox_server

```
matchbox_server
```

### Serve the game

```
WASM_SERVER_RUNNER_ADDRESS=0.0.0.0 cargo run --target wasm32-unknown-unknown
```

## License

Licensed under either of

* Apache License, Version 2.0
  ([LICENSE-APACHE] or https://www.apache.org/licenses/LICENSE-2.0)
* MIT license
  ([LICENSE-MIT] or https://opensource.org/license/MIT)

at your option.

[LICENSE-APACHE]: LICENSE-APACHE
[LICENSE-MIT]: LICENSE-MIT

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
