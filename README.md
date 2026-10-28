# baiseball-app

the bAIseball application. The public site is [league-office.app](https://league-office.app).

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

The office is two processes plus Postgres. The Rust API in `admin-api/` owns leagues, divisions, teams, lineups, rivalries, and the schedule. The Svelte app in `web/` calls that API and stores only the signed-in session. Neither process changes the plate-appearance simulator.

```bash
cd admin-api
cargo run
```

The API listens on `0.0.0.0:41742`. It reads `DATABASE_URL`, then `DATABASE_CONNECTION_STRING` (or the typo alias `DATABASE_CONNECTION_STRONG`). When those are unset, it uses a local Postgres database at `postgres://league:league@127.0.0.1:5432/league_office?sslmode=disable`. `compose.yaml` is the other way to run Postgres, on port 41743. Set `PUBLIC_APP_URL` and `OWNER_BOOTSTRAP_EMAIL` before `cargo run`. See `.do/app.yaml` for the App Platform services.

### DigitalOcean

The App Platform app `league-office` (id `652d5b1a-d4aa-4e0b-b41a-780b0c7fdebd`) tracks `main`. Auto-deploy on merge is handled by `.github/workflows/deploy-digitalocean.yml`.

Add a repository secret named `DIGITALOCEAN_ACCESS_TOKEN` (a DigitalOcean personal access token that can update App Platform apps). After that secret is set, every push to `main` creates a new deployment.

Native App Platform `deploy_on_push` needs the DigitalOcean GitHub integration to have access to `jeremyjbowers/baiseball-app`. Until that is connected in the DigitalOcean control panel, keep the git clone source in `.do/app.yaml` and rely on the workflow.

Manual deploy:

```bash
doctl apps create-deployment 652d5b1a-d4aa-4e0b-b41a-780b0c7fdebd --force-rebuild
```

For `sslmode=require`, the API encrypts the Postgres session and does not verify the server certificate (libpq behavior). For `sslmode=verify-ca` / `verify-full`, set `DATABASE_CA_CERT` (App Platform `${db.CA_CERT}`).

```bash
cd web
npm install
npm run dev
```

The app listens on `0.0.0.0:41741`. Open `http://127.0.0.1:41741` and sign in. Email uses Mailgun when configured. A text uses Twilio when the owner has saved a phone number. Without those providers, the sign-in link is shown on the next screen.

With the API running:

```bash
node admin-api/scripts/prove-admin.mjs
```

The route contract is `docs/admin-api.md` in the project notes.
