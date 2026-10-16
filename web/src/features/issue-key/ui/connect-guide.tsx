import { Check, CircleCheck, Copy, LoaderCircle } from "lucide-react";
import { useState } from "react";
import { Button, SegmentedControl } from "@/shared/ui";

const CLIENTS = [
  { value: "claude", label: "Claude Code" },
  { value: "codex", label: "Codex" },
  { value: "opencode", label: "opencode" },
  { value: "curl", label: "curl" },
] as const;

const WAYS = [
  { value: "accounts", label: "The gateway's accounts" },
  { value: "own", label: "Your own login" },
] as const;

type Client = (typeof CLIENTS)[number]["value"];
type Way = (typeof WAYS)[number]["value"];

function snippet(client: Client, way: Way, origin: string, key: string) {
  switch (client) {
    case "claude":
      return way === "own"
        ? `export ANTHROPIC_BASE_URL=${origin}\nexport ANTHROPIC_CUSTOM_HEADERS="x-gateway-key: ${key}"\nclaude`
        : `export ANTHROPIC_BASE_URL=${origin}\nexport ANTHROPIC_AUTH_TOKEN=${key}\nclaude`;
    case "codex":
      return way === "own"
        ? `# ~/.codex/config.toml\nmodel_provider = "goat"\n\n[model_providers.goat]\nname = "goat gateway"\nbase_url = "${origin}/v1"\nwire_api = "responses"\nrequires_openai_auth = true\nhttp_headers = { "x-gateway-key" = "${key}" }`
        : `# ~/.codex/config.toml\nmodel_provider = "goat"\n\n[model_providers.goat]\nname = "goat gateway"\nbase_url = "${origin}/v1"\nwire_api = "responses"\nenv_key = "GOAT_KEY"\n\n# then, in your shell\nexport GOAT_KEY=${key}`;
    case "opencode":
      return `// opencode.json\n{\n  "provider": {\n    "goat": {\n      "npm": "@ai-sdk/anthropic",\n      "options": { "baseURL": "${origin}/v1", "apiKey": "${key}" },\n      "models": { "claude-sonnet-5-5": {} }\n    }\n  }\n}`;
    case "curl":
      return `curl ${origin}/v1/messages \\\n  -H "x-api-key: ${key}" \\\n  -H "anthropic-version: 2023-06-01" \\\n  -H "content-type: application/json" \\\n  -d '{"model":"claude-sonnet-5-5","max_tokens":64,"messages":[{"role":"user","content":"hi"}]}'`;
  }
}

function caveat(client: Client, way: Way) {
  if (way === "own" && (client === "opencode" || client === "curl")) {
    return "This client sends no login of its own here, so the gateway's accounts serve it either way.";
  }
  if (way === "own") {
    return "The client keeps using the plan it is signed in to. The gateway passes that login to its provider untouched and never stores it; the key only says who you are.";
  }
  switch (client) {
    case "claude":
      return "Claude Code may ask you to log in the first time; choose to use the API key from the environment.";
    case "codex":
      return "Codex speaks only the Responses format, so the base URL ends in /v1.";
    case "opencode":
      return "The AI SDK adds /messages itself, so the base URL ends in /v1 even for Anthropic models.";
    case "curl":
      return "Any client that speaks Messages, Responses or Chat Completions works the same way.";
  }
}

export function ConnectGuide({
  secret,
  firstSeen,
}: {
  secret?: string;
  firstSeen?: number | null;
}) {
  const [client, setClient] = useState<Client>("claude");
  const [way, setWay] = useState<Way>("accounts");
  const [copied, setCopied] = useState(false);
  const text = snippet(client, way, window.location.origin, secret ?? "gwk_…");

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <SegmentedControl
          label="Client"
          value={client}
          choices={CLIENTS}
          onChange={(next) => {
            setClient(next);
            setCopied(false);
          }}
        />
        <SegmentedControl
          label="Which plan pays"
          value={way}
          choices={WAYS}
          onChange={(next) => {
            setWay(next);
            setCopied(false);
          }}
        />
      </div>
      <div className="relative">
        <pre className="mono m-0 overflow-x-auto rounded-lg hairline border-line bg-base px-3 py-2.5 text-mini leading-relaxed text-ink-title">
          {text}
        </pre>
        <Button
          tone="ghost"
          size="small"
          className="absolute top-1.5 right-1.5"
          onClick={() => {
            void navigator.clipboard.writeText(text).then(
              () => setCopied(true),
              () => setCopied(false),
            );
          }}
        >
          {copied ? <Check strokeWidth={1.75} /> : <Copy strokeWidth={1.75} />}
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
      <p className="m-0 text-mini text-ink-secondary">{caveat(client, way)}</p>
      {firstSeen === undefined ? null : (
        <p
          className={
            firstSeen
              ? "m-0 flex items-center gap-2 text-small text-good-text"
              : "m-0 flex items-center gap-2 text-small text-ink-secondary"
          }
          aria-live="polite"
        >
          {firstSeen ? (
            <>
              <CircleCheck className="size-4" strokeWidth={1.75} />
              Connected. The first request with this key just arrived.
            </>
          ) : (
            <>
              <LoaderCircle className="size-4 animate-spin" strokeWidth={1.75} />
              Waiting for the first request with this key…
            </>
          )}
        </p>
      )}
    </div>
  );
}
