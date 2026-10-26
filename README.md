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

## Step a game

```bash
cargo run --bin serve
```

The server listens on `0.0.0.0:41731`. Open `http://127.0.0.1:41731`. Start reads `fixtures/game.json` and keeps that game in memory until the process stops.

- `POST /api/games` opens the fixture and returns an id and the state.
- `GET /api/games/:id` reads that saved state.
- `POST /api/games/:id/plate-appearances` resolves one plate appearance. The body may be empty, or `{"call":"bunt"}`, `intentional_walk`, `steal`, or `hit_and_run`.

Leaving the page does not play the next batter. The page is one person stepping the game.

## Later

AWS Lambda, DigitalOcean, and App Engine are later places this can run. This slice does not implement them. It runs with Cargo.

## League office

The office is two processes. The Rust API in `admin-api/` owns leagues, divisions, teams, lineups, owners, and magic-link tokens in SQLite. The Svelte app in `web/` calls that API and stores only the signed-in session. Neither one changes the plate-appearance simulator.

```bash
cd admin-api
cargo run
```

The API listens on `0.0.0.0:41742`.

```bash
cd web
npm install
npm run dev
```

The app listens on `0.0.0.0:41741`. Open `http://127.0.0.1:41741`. Sign in with an email address. This slice has no mail provider: the magic link is printed in the API log and shown on the confirmation screen.

With the API running:

```bash
node admin-api/scripts/prove-admin.mjs
```

The route contract is `docs/admin-api.md` in the project notes.
