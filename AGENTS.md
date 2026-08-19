# AGENTS.md

A self-hosted gateway in front of AI accounts you already have. Three request formats in,
whatever the provider serves out. Rust workspace plus a React console embedded in the binary.

Read `README.md` for what the product is. This file is how to work on it.

## Repository layout

- `crates/goat-gateway-wire` — formats, translation, envelopes, digests. Pure functions over
  bytes. **No network, no database, no async.**
- `crates/goat-gateway` — HTTP, routing, account selection, storage, the API the console reads.
- `web` — the console. Vite + React + Tailwind v4, Feature-Sliced.

Each of the three has its own `AGENTS.md` with rules specific to it. Read the nearest one.

## Setup

Rust from `rust-toolchain.toml` (stable, with `rustfmt` and `clippy`) and Node 20+. No other
tooling. `cargo build` will run `npm install` and build the console; set `GOAT_SKIP_WEB_BUILD=1`
to skip that while working on Rust only.

## Commands

```sh
cargo test --workspace                            # all of it
cargo test -p goat-gateway-wire                   # translators, no I/O, fast
cargo test -p goat-gateway --test routing         # end to end through a fake upstream
cargo test -p goat-gateway-wire --test losslessness
cargo test -p goat-gateway-wire a_tool_call_keeps_the_id_it_was_given   # one test by name

cargo clippy --workspace --all-targets            # must print nothing
cargo fmt

npm --prefix web run check                        # tsc, then the layering check
npm --prefix web run build
npm --prefix web run dev                          # proxies /api to 127.0.0.1:8787
```

Run the narrowest relevant check first, then widen in proportion to what you touched.

To exercise it by hand:

```sh
GOAT_DATA_DIR=/tmp/goat GOAT_ADMIN_KEY=gwa_dev GOAT_PORT=8791 cargo run -p goat-gateway
```

## Rules

These are not style preferences. Breaking one produces a bug that surfaces a turn later, in
someone else's terminal.

- **Never rewrite a tool id.** Anthropic's `toolu_…` satisfies what Chat Completions accepts
  and OpenAI's `call_…` satisfies Anthropic's pattern. A rewritten id makes the conversation
  unresumable on the next turn.
- **Never invent a signature.** Reasoning a provider did not sign must not be made to look
  signed. Drop it and record the drop.
- **Nothing disappears silently.** Whatever a translation cannot carry goes into
  `Mapping::dropped` with a reason. Each translator holds an explicit `CARRIED` list of the
  top-level fields it consumes; everything else is reported as dropped. Adding a field to that
  list without handling it turns a visible loss into a silent one.
- **Passthrough is byte for byte.** When the arriving format is one the provider serves, do not
  reserialize the body. Cache breakpoints are the only edit, recorded as `BodyEdit`s, and
  `Record::verify` replays them before the request goes out.
- **Never park a healthy account.** A 429 carrying no reported limit and no `Retry-After` is a
  transient throttle or an identity rejection, not spent quota.
- **No price means no cost, never zero.** A model with no declared price reports nothing.
- **Never poll a provider on a timer.** Quota is read from responses to requests that were
  going to happen anyway; endpoint-based quota is asked at most once a minute per account, on
  the back of real traffic.

## Adding things

- **A provider** is a TOML entry in `crates/goat-gateway/src/provider/builtin.toml`. No Rust.
  If it needs Rust, the declaration is missing a field — add the field.
- **A format** needs two translators, to and from Messages, plus entries in
  `crates/goat-gateway/src/relay_path.rs`. Everything else composes through the hub. Do not
  add a direct translator for a pair a two-hop path already covers.
- **A screen** is a slice under `web/src/pages` with an `index.ts`.

## Code style

- **No comments.** There are none in this repo and none should be added. If something needs
  explaining, rename it or restructure it until it does not. The one exception is the boundary
  note in `crates/goat-gateway-wire/Cargo.toml`; leave it there.
- Names read as prose: `pinned_account`, `worst_window`, `mind_the_rest`. Not `get_x`, `do_y`,
  `handle_z`, `process`, `manager`, `util`.
- Errors are written to the person reading them: full sentences, naming what to do next.
- No `unwrap` or `expect` on anything that can fail at runtime. Mutex poisoning is the
  exception.
- No `unsafe` — the workspace forbids it.
- TypeScript: no type assertions. No `as`, no `!`. Narrow by checking the field, or use an
  `as const` tuple so the index is known.

## Testing

- Name a test as a sentence stating what must be true:
  `a_full_model_bucket_does_not_take_the_whole_account_away`. Give an assertion a message when
  the reason it matters is not obvious from the name.
- Assert relationships, not fixed output — that is what survives the translators changing.
  `crates/goat-gateway-wire/tests/losslessness.rs` is the model: nothing vanishes without
  being recorded, a record that cannot be replayed refuses to verify, and where the network
  split the bytes changes nothing about what the client receives.
- New behaviour needs a test that would have failed before it. A bug fix needs the test first.
- Do not test values that are statically defined, and do not add tests for logic you removed.

## Releasing

Tag a commit `v0.1.0`-style and push the tag. `release.yml` builds the image, pushes it to
`ghcr.io/goat-agent/goat-gateway` under the semver tags and `latest`, and opens a GitHub
release whose notes are generated from the merged pull requests.

## Commits

The subject is a sentence saying what changed, in the product's terms, not the code's:
`Tell a spent account apart from a busy one`, not `fix: rate limit handling`. No type prefix,
no scope, no trailing period.

The body explains why the change was necessary and what it prevents — the reasoning that will
not be visible in the diff a year from now. Wrap at 78 columns. No `Co-Authored-By` or
assistant attribution.

One commit, one reason to exist. A refactor and a fix are two commits even in the same file.

## Before you finish

- `cargo test --workspace` passes.
- `cargo clippy --workspace --all-targets` prints nothing.
- `cargo fmt` leaves nothing to change.
- `npm --prefix web run check` passes if you touched `web`.
- Anything you could not verify is stated plainly, with the reason.
