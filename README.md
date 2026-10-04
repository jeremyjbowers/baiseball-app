# Baseball engine

One regular-season game. The crate is a library plus a `play` binary. A fixed lineup, fixture ratings, and a minimal computer manager play nine innings. The designated hitter is on, so pitchers do not hit. If the game is tied after nine, a runner starts on second. A postseason flag exists and leaves second empty.

The library is the state machine, the stateless at-bat simulator, and that computer manager. The simulator stores nothing. The state machine keeps the play-by-play, the three player buckets, and pitcher fatigue, and it rejects four outs, a player sent back in, and a pitcher left on the mound past the fatigue line.

## Run

Install Rust if `cargo` is not already available:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
. "$HOME/.cargo/env"
```

From the repository root:

```bash
cargo build
cargo test
cargo run --bin play
```

`cargo run --bin play` reads `fixtures/game.json` and plays one game. The game is a new draw every time.

## Later

AWS Lambda, DigitalOcean, and App Engine are later places this can run. This slice does not implement them. It runs with Cargo.
