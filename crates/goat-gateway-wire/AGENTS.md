# AGENTS.md — goat-gateway-wire

Formats and what happens between them. Every function here is pure over bytes.

## The boundary

**No `reqwest`, no `rusqlite`, no `tokio`.** Not "avoid" — the crate can be tested exhaustively
because it cannot do I/O, and one async dependency ends that. If something here appears to need
the network, it belongs in `goat-gateway` instead.

`Cargo.toml` says the same above its dependency list. Leave that note there; it is the only
comment in the repository.

## Commands

```sh
cargo test -p goat-gateway-wire
cargo test -p goat-gateway-wire --test losslessness              # the properties
cargo test -p goat-gateway-wire nothing_ever_invents_a_signature # one test by name
```

## Layout

- `sse.rs` — frame reassembly. Chunk boundaries are arbitrary and it must not care.
- `edit.rs` — `BodyEdit`, `apply`, `Record::verify`. Every change replayable from its record.
- `cache.rs` — where cache breakpoints go, and where they do not.
- `envelope.rs` — opaque provider state resealed under our key, carrying provenance.
- `mapping.rs` — what moved, what was added, what was dropped and why.
- `meter.rs` — reads usage off a stream without touching a byte of it.
- `<from>_to_<to>.rs` — one file per direction: request translation, then the stream translator.

## Translating a request

- Every translator holds a `CARRIED` list of the top-level fields it consumes.
  `Mapping::mind_the_rest` reports everything else as dropped. Listing a field there without
  handling it converts a real loss into a silent one — worse than not having the list.
- A block type with no counterpart is an error, not a drop. Refuse the request rather than send
  something that quietly means less than what was asked for.
- A field that is recognised but cannot be honoured — `previous_response_id` reaching a
  provider that holds no state — is a drop with a reason, not an omission.
- Tool ids travel unchanged. Signatures are never invented. Both have their own tests, because
  several other implementations get them wrong.

## Translating a stream

- A stream translator is `push(&[u8]) -> Vec<u8>` plus `finish()`. Output must be identical
  regardless of where chunks were split; `where_the_network_split_the_bytes_changes_nothing`
  checks seven splits including one byte at a time.
- A stream that dies mid-flight must say so. Ending quietly makes a failure look like an
  answer.
- Translators that can also serve a non-streaming client expose `assembled()`, built from the
  same events they emitted, so the two cannot disagree.
- Never emit a field the source did not provide. An empty string is not the same as absent.
