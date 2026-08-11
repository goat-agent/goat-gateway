import { drop, useResource } from "@/shared/api";
import { show } from "@/shared/lib";
import {
  Body,
  Button,
  Head,
  Nothing,
  Numeric,
  NumericHead,
  Page,
  Panel,
  PanelHead,
  Row,
  Table,
} from "@/shared/ui";

type Price = {
  provider: string;
  model: string;
  input_per_mtok_micros: number;
  output_per_mtok_micros: number;
  cache_read_per_mtok_micros: number;
  cache_write_per_mtok_micros: number;
};

export function SettingsPage() {
  const models = useResource<Record<string, string[]>>("/api/models");
  const prices = useResource<{ prices: Price[]; as_of: string }>("/api/pricing");

  const providers = Object.entries(models.data ?? {});

  return (
    <Page title="Settings">
      <Panel>
        <PanelHead title="Providers" note="declared in config.toml next to the database, or built in" />
        {providers.length === 0 ? (
          <Nothing says="No provider is declared, which should not be possible." />
        ) : (
          <Table>
            <Head>
              <Row>
                <th>Provider</th>
                <th>Models it declares</th>
              </Row>
            </Head>
            <Body>
              {providers.map(([provider, names]) => (
                <Row key={provider}>
                  <td className="text-ink">{provider}</td>
                  <td className="whitespace-normal text-ink-secondary">
                    {names.length > 0 ? names.join(", ") : "none — anything routes through untouched"}
                  </td>
                </Row>
              ))}
            </Body>
          </Table>
        )}
      </Panel>

      <Panel>
        <PanelHead
          title="Prices"
          note={prices.data ? `as of ${prices.data.as_of}, per million tokens` : undefined}
        />
        {prices.data && prices.data.prices.length > 0 ? (
          <Table>
            <Head>
              <Row>
                <th>Model</th>
                <NumericHead>Input</NumericHead>
                <NumericHead>Output</NumericHead>
                <NumericHead>Cache read</NumericHead>
                <NumericHead>Cache write</NumericHead>
              </Row>
            </Head>
            <Body>
              {prices.data.prices.map((price) => (
                <Row key={`${price.provider}-${price.model}`}>
                  <td className="text-ink">{price.model}</td>
                  <Numeric>{show.money(price.input_per_mtok_micros)}</Numeric>
                  <Numeric>{show.money(price.output_per_mtok_micros)}</Numeric>
                  <Numeric>{show.money(price.cache_read_per_mtok_micros)}</Numeric>
                  <Numeric>{show.money(price.cache_write_per_mtok_micros)}</Numeric>
                </Row>
              ))}
            </Body>
          </Table>
        ) : (
          <Nothing says="No model declares a price. Requests will show no cost rather than a cost of zero." />
        )}
      </Panel>

      <Panel>
        <PanelHead title="This session" />
        <div className="flex items-center justify-between gap-4 p-3">
          <p className="m-0 text-small text-ink-secondary">
            Signing out clears the cookie in this browser. The admin key itself does not change.
          </p>
          <Button onClick={() => drop("/api/session").finally(() => window.location.reload())}>
            Sign out
          </Button>
        </div>
      </Panel>
    </Page>
  );
}
