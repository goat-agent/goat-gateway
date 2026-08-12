# Providers

Every provider the gateway knows is a declaration, not code. The built-in set lives in
`crates/goat-gateway/src/provider/builtin.toml`. To change one, or add one, drop a
`config.toml` next to the database and declare it there — the key you write replaces the
built-in of the same name, and anything you leave out stays as it was.

The gateway refuses to start on a `config.toml` it cannot parse, including a misspelled
field. A provider list that half-applied would be worse than one that did not start.

## What a declaration says

```toml
[provider.example]
label = "Example"
key = "bearer"
limits = "headers"
endpoints = [{ wire = "messages", url = "https://api.example.com/v1/messages" }]
headers = { "anthropic-version" = "2023-06-01" }

[[provider.example.models]]
name = "example-large"
limit_scope = "large"
cache_min_tokens = 1024
price = { input = 3000000, output = 15000000, cache_read = 300000, cache_write = 3750000 }
thinking = "adaptive"
max_tokens = 32000
mid_conversation_system = false
```

`endpoints` is the whole routing story. A request arriving in a format the provider already
serves is passed through untouched; one arriving in another format is translated, and only
into a format some translator can reach. Nothing is flagged as passthrough — it is what
happens when the two formats match.

`key` is how an API key is presented: `bearer` puts it in `Authorization`, `x_api_key` puts
it in `x-api-key`. Getting this wrong looks exactly like a bad key.

`limits` is where quota comes from. `headers` reads it off responses the gateway is already
making. `none` means this provider does not say. `{ endpoint = { … } }` names a place to ask,
and where the numbers are in the answer:

```toml
limits = { endpoint = { url = "https://api.example.com/usage", windows = [
  { label = "weekly", used_percent = "/data/weekly/used", resets_at = "/data/weekly/reset_at" },
  { label = "5h", scope = "large", used_percent = "/data/five_hour/used", resets_in_seconds = "/data/five_hour/left" },
] } }
```

The pointers are JSON pointers into whatever the endpoint returns. A percentage may be given
as 0–1 or 0–100; either is read correctly. A reset may be an absolute moment or seconds
remaining. Declare the URL without any windows and the gateway will still ask, keep what came
back, and show it on the overview so the pointers can be written from the real answer rather
than guessed. Asking happens on the back of real traffic, at most once a minute per account,
because these endpoints rate-limit aggressively and without a Retry-After.

`price` is in micro-dollars per million tokens. Leave it out and requests to that model show
no cost rather than a cost of zero — a model whose price we do not know is not free.

`thinking`, `max_tokens`, and `mid_conversation_system` are only read when a request has to
be translated into this provider's format. A provider that is only ever passed through needs
a model name and nothing else.

## Formats

Every provider is reached from every door. A request arriving in a format the provider already
serves is passed through untouched; anything else is translated, and where there is no direct
translator the gateway goes through Messages — a Responses request reaches a Chat-only coding
plan as Responses to Messages to Chat, and the reply comes back the same way reversed.

That means a new format costs two translators, to and from Messages, rather than one for every
other format that exists.

## The built-in set

| Provider | Serves | Key | Notes |
|---|---|---|---|
| `anthropic` | Messages | `x-api-key` | Quota in response headers, per model bucket |
| `openai` | Responses, Chat Completions | Bearer | Quota in response headers, per limit id |
| `zai` | Messages | Bearer | GLM Coding Plan, quota from an endpoint |
| `kimi` | Chat Completions | Bearer | Kimi Coding Plan, quota from an endpoint |
| `moonshot` | Messages, Chat Completions | Bearer | Pay-per-token, a different host from `kimi` |

## Why a coding plan is its own provider

`kimi` and `moonshot` are the same vendor and different products. The coding plan is billed
against a refreshing quota and answers on `api.kimi.com/coding`; the pay-per-token API is
billed per token and answers on `api.moonshot.ai`. A coding-plan key sent to the
pay-per-token host does not fail loudly, and neither does the reverse.

The same split applies wherever a vendor's plan, protocol, or region changes the host. Treat
"which URL does this key work against" as the thing that defines a provider, not the company
name.

Two more things follow from that. Kimi's coding plan turns away clients whose
`User-Agent` is not one it recognises, and it does so with a 429 — which reads exactly like a
quota that has run out. The gateway only parks an account when the response actually reports
a limit or asks it to wait, so a rejection wearing a 429 costs nothing. And Z.ai does not
always return usage in Anthropic's shape, so a request through it may record no token counts;
that is the provider being quiet, not the gateway losing them.

## Adding one

Point a declaration at whatever the vendor calls Anthropic-compatible or OpenAI-compatible,
give it the models you actually use, and register an account. If the vendor speaks a format
the gateway already serves, that is the whole job. If it does not, the gateway will say so
rather than send something it has not been taught to translate.
