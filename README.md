# goat gateway

Register the AI accounts you already have. Point every client at one address.

```
      what arrives                      what it reaches

  POST /v1/messages          ─┐      ┌─ Anthropic          (Messages)
  POST /v1/responses         ─┼─ ▸ ──┼─ OpenAI             (Responses, Chat)
  POST /v1/chat/completions  ─┘      ├─ Z.ai GLM plan      (Messages)
                                     ├─ Kimi coding plan   (Chat)
                                     └─ anything you declare
```

A request in a format the provider already serves goes through **byte for byte**. Anything
else is translated, and every field that could not come along is written down where you can
see it. Accounts have names, not owners; a request goes to whichever one can serve it.

## Running it

```sh
cargo run -p goat-gateway
```

It prints an admin key once, opens on `127.0.0.1:8787`, and keeps everything in
`~/.goat-gateway`. Open the address, add an account, issue yourself a key, then point a client
at it:

```sh
ANTHROPIC_BASE_URL=http://127.0.0.1:8787 ANTHROPIC_AUTH_TOKEN=gwk_… claude
OPENAI_BASE_URL=http://127.0.0.1:8787/v1 OPENAI_API_KEY=gwk_… codex
```

| Variable | |
|---|---|
| `GOAT_HOST`, `GOAT_PORT` | where to listen. Off loopback, `GOAT_ADMIN_KEY` is required |
| `GOAT_ADMIN_KEY` | set it yourself instead of having one generated |
| `GOAT_DATA_DIR` | where the database and `config.toml` live |
| `GOAT_MASTER_KEY` | 64 hex characters, instead of `master.key` in the data directory |

Credentials are encrypted with the master key before they touch the database, and the key file
is written `0600`. Nothing is stored in the clear.

## Formats

Messages is the hub. A pair with no direct translator goes through it — a Responses request
reaches a Chat-only coding plan as Responses → Messages → Chat, and the reply comes back the
same way reversed. A new format costs two translators rather than one per format that exists.

Two things never happen, in either direction. **A tool id is never rewritten**, because a
rewritten id makes the conversation unresumable one turn later. **A signature is never
invented**, because reasoning that a provider did not sign cannot be made to look like
reasoning it did — every implementation that fabricates one fails on the following turn.

Anything the gateway cannot carry across is recorded against the request rather than dropped
quietly, including fields no public schema mentions.

## Providers

Providers are declarations, not code. The built-in list is
`crates/goat-gateway/src/provider/builtin.toml`; a `config.toml` beside the database replaces
any entry by name and leaves the rest alone. A file that will not parse stops startup, because
a provider list that half-applied is worse than one that never loaded.

```toml
[provider.example]
label = "Example"
key = "bearer"                     # or "x_api_key" — getting it wrong looks like a bad key
limits = "headers"                 # or "none", or { endpoint = { … } }
endpoints = [{ wire = "messages", url = "https://api.example.com/v1/messages" }]
headers = { "anthropic-version" = "2023-06-01" }

[[provider.example.models]]
name = "example-large"
limit_scope = "large"              # which quota bucket it spends
cache_min_tokens = 1024            # below this, caching is not worth a breakpoint
price = { input = 3000000, output = 15000000, cache_read = 300000, cache_write = 3750000 }
thinking = "adaptive"              # only read when a request must be translated into this
max_tokens = 32000                 # format; a passthrough model needs a name and nothing else
```

`endpoints` is the whole routing story: what a provider serves is what it is passed, and
everything else is a translation. Prices are micro-dollars per million tokens — leave one out
and requests to that model show no cost rather than a cost of zero.

Quota comes from response headers, from an endpoint, or not at all. For an endpoint, declare
where the numbers are:

```toml
limits = { endpoint = { url = "https://api.example.com/usage", windows = [
  { label = "weekly", used_percent = "/data/weekly/used", resets_at = "/data/weekly/reset_at" },
] } }
```

Those are JSON pointers into whatever comes back. Declare the URL with no windows and the
gateway still asks, keeps the answer, and shows it, so the pointers can be written from the
real shape instead of guessed. It only ever asks on the back of real traffic, at most once a
minute per account.

A vendor's plan is its own provider when it answers on its own host. A coding-plan key sent to
the pay-per-token host fails quietly, and so does the reverse, which is why `kimi` and
`moonshot` are separate entries for the same company.

## What it knows

Quota is read from the responses to requests that were going to happen anyway, never by
polling — so it is sometimes stale, and the screen says when it was last seen. A provider that
never reports quota reads differently from one that has not been asked yet.

Latency is measured over successful requests only: an expired account answering 401 in three
milliseconds otherwise makes a broken provider look like a fast one. Cost is reported next to
how many requests carried a published price, so a total can say how much of the story it is.

Two numbers here that comparable tools do not show, because only something pooling accounts
can know them: how many requests are in flight, and how much quota is left.

## Building

```sh
cargo test --workspace          # 283 tests
cargo clippy --workspace --all-targets
npm --prefix web run check      # types, and the layering the console claims to have
```

`cargo build --release` builds the console and embeds it. `GOAT_SKIP_WEB_BUILD=1` skips that
when you are only working on the Rust side.

## Licence

MIT or Apache-2.0, at your option.
