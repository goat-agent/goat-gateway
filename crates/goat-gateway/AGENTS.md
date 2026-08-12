# AGENTS.md — goat-gateway

HTTP, routing, account selection, storage, and the API the console reads.

## Commands

```sh
cargo test -p goat-gateway
cargo test -p goat-gateway --test routing        # every door, through a fake upstream
cargo test -p goat-gateway --test passthrough    # the bytes must not change
cargo test -p goat-gateway a_pinned_account_is_used_even_when_it_is_not_the_calmest
```

## The path a request takes

`serve::dispatch` is the only entry. It admits the request — route it, pick an account, prepare
credentials — then either passes it through or walks a translation path.

- `serve.rs` — admit, dispatch, passthrough, and the shape of an error per format.
- `translate.rs` — one handler for every translating pair, walking `relay_path`.
- `relay_path.rs` — which hops get from one format to another. Messages is the hub.
- `provider/` — the declarations, and routing a model to a provider.
- `pool.rs` — which account serves this, and why the others cannot.
- `limits.rs` — what a provider said about quota, from headers or from an answer.
- `quota.rs` — asking providers that only report when asked.
- `turns.rs` — a turn state pins to the account that minted it.
- `probe.rs` — the connection test: a real request, and what came back.
- `store/` — SQLite. `insight.rs` is everything the console asks about history.

## Routing

- A model belongs to the provider that declares it.
- A model nobody declares goes to the one registered provider that speaks the arriving format.
  Speaking it natively beats reaching it by translation; that is what keeps a mixed pool
  unambiguous. Two candidates is an error naming both, not a coin toss.
- Passthrough is not a flag. It is what happens when `route.endpoint.wire == incoming.wire`.

## Choosing an account

`pool::pick` takes a `Want`.

- A **pin** must be honoured or the request fails. Reasoning state and turn state belong to one
  account; sending them elsewhere is an error, not a worse answer.
- A **preference** gives way. Conversation affinity keeps a cache warm, and a warm cache is
  worth less than an answer.
- Quota is per `(account, bucket)`. Buckets are discovered from header names, never enumerated
  — a full Fable bucket must not take Sonnet capacity away.
- Never remove an account on a percentage. Requests succeed at 100% used. Only a reported limit
  or an explicit wait takes one out of the pool.

## Recording

- A row is written before the request is sent. That is what makes in-flight a real number
  rather than an inferred one.
- The stream's end decides the final status, not the response headers.
- A row still in flight long after any plausible request reads as abandoned, not running.
- Failures keep what the provider said. Relaying an error without capturing its body leaves the
  only explanation in someone else's terminal.

## Storage

- Secrets are encrypted before they reach the database. Nothing is stored in the clear, and
  `the_secret_is_not_stored_in_the_clear` asserts it.
- Schema changes are a new entry in `store/schema.rs`. Never edit an existing migration.
