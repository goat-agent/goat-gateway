import { Link } from "react-router-dom";
import { Pencil, RotateCcw, Trash2 } from "lucide-react";
import {
  Badge,
  Body,
  Button,
  Cell,
  Count,
  Detail,
  Head,
  Inspector,
  Nothing,
  Properties,
  Property,
  Row,
  Table,
  type Tone,
} from "@/shared/ui";
import { WIRE_LABELS, type ProviderEntry } from "@/entities/provider";
import { money, tokens, UNKNOWN } from "@/shared/lib/format";

export type Price = {
  provider: string;
  model: string;
  input_per_mtok_micros: number;
  output_per_mtok_micros: number;
  cache_read_per_mtok_micros: number;
  cache_write_per_mtok_micros: number;
};

export type Pricing = { prices: Price[]; as_of: string };

export const ORIGINS: Record<ProviderEntry["origin"], { label: string; tone: Tone }> = {
  built_in: { label: "Built in", tone: "neutral" },
  config: { label: "config.toml", tone: "info" },
  overridden: { label: "Overridden", tone: "warning" },
};

const QUOTA: Record<ProviderEntry["limits"], string> = {
  headers: "Read from its response headers",
  endpoint: "Asked, at most once a minute",
  none: "Not reported",
};

export function ways(provider: ProviderEntry) {
  return [
    provider.sign_in ? "plan" : undefined,
    provider.api_key ? "API key" : undefined,
    provider.carries_own_login ? "client's own login" : undefined,
  ].filter((way) => way !== undefined);
}

type Listed = {
  name: string;
  thinking: string | null;
  max_tokens: number | null;
  price: Price | undefined;
};

export function ProviderInspector({
  id,
  provider,
  loaded,
  pricing,
  writable,
  onEdit,
  onRemove,
  onClose,
}: {
  id: string;
  provider: ProviderEntry | undefined;
  loaded: boolean;
  pricing: Pricing | undefined;
  writable: boolean;
  onEdit: (provider: ProviderEntry) => void;
  onRemove: (provider: ProviderEntry) => void;
  onClose: () => void;
}) {
  if (!provider) {
    return (
      <Inspector kicker="Provider" title={id} onClose={onClose}>
        {loaded ? <Nothing says="No provider with this id is declared." /> : null}
      </Inspector>
    );
  }

  const prices = (pricing?.prices ?? []).filter((price) => price.provider === provider.id);
  const declared: Listed[] = provider.models.map((model) => ({
    name: model.name,
    thinking: model.thinking,
    max_tokens: model.max_tokens,
    price: prices.find((price) => price.model === model.name),
  }));
  const priceOnly: Listed[] = prices
    .filter((price) => !provider.models.some((model) => model.name === price.model))
    .map((price) => ({ name: price.model, thinking: null, max_tokens: null, price }));
  const models = [...declared, ...priceOnly];

  return (
    <Inspector
      kicker={<span className="mono">{provider.id}</span>}
      title={provider.label}
      subtitle={
        <>
          <Badge tone={ORIGINS[provider.origin].tone}>{ORIGINS[provider.origin].label}</Badge>
          <span>{provider.endpoints.map((endpoint) => WIRE_LABELS[endpoint.wire]).join(", ")}</span>
        </>
      }
      actions={
        writable ? (
          <>
            <Button onClick={() => onEdit(provider)}>
              <Pencil strokeWidth={1.75} />
              {provider.origin === "built_in" ? "Override in config.toml" : "Edit declaration"}
            </Button>
            {provider.origin === "built_in" ? null : (
              <Button onClick={() => onRemove(provider)}>
                {provider.origin === "overridden" ? (
                  <RotateCcw strokeWidth={1.75} />
                ) : (
                  <Trash2 strokeWidth={1.75} />
                )}
                {provider.origin === "overridden" ? "Restore built-in" : "Remove"}
              </Button>
            )}
          </>
        ) : undefined
      }
      onClose={onClose}
    >
      <Detail>
        <Properties>
          <Property name="Served by" value={ways(provider).join(", ") || UNKNOWN} />
          {provider.sign_in ? (
            <Property name="Sign-in" value={`${provider.sign_in.label} · ${provider.sign_in.modes.join(", ")}`} />
          ) : null}
          <Property
            name="Key goes in"
            value={provider.key === "x_api_key" ? "x-api-key" : "Authorization: Bearer"}
          />
          <Property name="Quota" value={QUOTA[provider.limits]} />
          <Property
            name="Accounts"
            value={
              provider.accounts > 0 ? (
                <Link to="/accounts" className="hover-fade text-link hover:underline">
                  {provider.accounts} registered
                </Link>
              ) : (
                "none yet"
              )
            }
          />
        </Properties>
      </Detail>

      <Detail title="Endpoints">
        <ul className="m-0 flex list-none flex-col gap-2 p-0">
          {provider.endpoints.map((endpoint) => (
            <li key={endpoint.wire} className="flex min-w-0 flex-col gap-0.5">
              <span className="text-small text-ink">{WIRE_LABELS[endpoint.wire]}</span>
              <span className="mono truncate text-ink-secondary select-text" title={endpoint.url}>
                {endpoint.url}
              </span>
            </li>
          ))}
        </ul>
      </Detail>

      <Detail
        flush
        title={`Models ${models.length}`}
        aside={pricing ? `price per million tokens, as of ${pricing.as_of}` : undefined}
      >
        {models.length === 0 ? (
          <p className="m-0 px-5 text-small text-ink-secondary">
            None declared. A model nobody declares still reaches it when it is the only provider that
            speaks the arriving format.
          </p>
        ) : (
          <Table className="table-fixed [&_td]:h-[var(--row-compact)]!">
            <Head>
              <Row>
                <th>Name</th>
                <Count className="w-20">Max out</Count>
                <Count className="w-18">Input</Count>
                <Count className="w-18">Output</Count>
              </Row>
            </Head>
            <Body>
              {models.map((model) => (
                <Row key={model.name}>
                  <td className="truncate" title={model.thinking ? `thinking: ${model.thinking}` : undefined}>
                    <span className="mono text-ink">{model.name}</span>
                  </td>
                  <Cell className="text-ink-secondary">{tokens(model.max_tokens)}</Cell>
                  {model.price ? (
                    <>
                      <Cell>{money(model.price.input_per_mtok_micros)}</Cell>
                      <Cell>{money(model.price.output_per_mtok_micros)}</Cell>
                    </>
                  ) : (
                    <td colSpan={2} className="text-right text-mini text-ink-muted">
                      no price
                    </td>
                  )}
                </Row>
              ))}
            </Body>
          </Table>
        )}
      </Detail>
    </Inspector>
  );
}
