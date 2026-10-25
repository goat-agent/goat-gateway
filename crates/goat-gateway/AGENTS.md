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
- `provider/` — the declarations, and which providers can serve a model.
- `pool.rs` — which account serves this, and why the others cannot.
- `limits.rs` — what a provider said about quota, from headers or from an answer.
- `quota.rs` — asking providers that only report when asked.
- `turns.rs` — a turn state pins to the account that minted it.
- `probe.rs` — the connection test: a real request, and what came back.
- `store/` — SQLite. `insight.rs` is everything the console asks about history.

## Routing

- A model is served by every provider that declares it and has an account the caller can use.
  Their accounts are one pool: a ChatGPT plan and an OpenAI key that both declare `gpt-5` take
  turns, and a spent one hands the model to the other.
- A model nobody declares goes to the one registered provider that speaks the arriving format.
  Speaking it natively beats reaching it by translation. When several remain, the one whose
  login the client carried wins, then one the caller owns an account with. Past that, two
  candidates is an error naming both, not a coin toss.
- Each candidate carries its own route. Passthrough is not a flag. It is what happens when
  `route.endpoint.wire == incoming.wire` for the candidate being tried.

## Who is asking

- A gateway key (`gwk_`) names a person. It may arrive as the bearer or in `x-gateway-key`.
- When it arrives in `x-gateway-key`, the client's own `authorization` or `x-api-key` is a
  **carried** login. It is matched against the providers' `carried` declarations, sent only to
  the hosts declared there, never stored, and refused with a 401 if nobody declared it.
- A key is a label. It says who is calling and, by convention, which program. It never changes
  which account serves a request.
- The console signs people in with sessions (`gws_`, hashed in `sessions`). Every session is a
  person. The admin key only recovers one: given with a name and a new password, it sets that
  password, makes that person an admin, and signs them in. A name nobody has makes a new admin.

## Choosing an account

`pool::candidates` takes a `Want` and returns them in order; `pool::pick` is the first.

- Only accounts the caller owns or that are shared with everyone are candidates. A login the
  client carried comes before all of them.
- Order: a pin, then the caller's own, then the one that served this conversation for this
  person, then the least quota pressure.
- When a candidate cannot serve and no byte has reached the client, the request goes once more
  to the next one. Cannot serve means a declared refusal (`[[provider.x.refusals]]`), which
  parks nothing and is not remembered, or an answer that parks the account: a reported limit,
  or a sign-in that expired. One request is one row; earlier attempts are kept in `attempts`.

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
