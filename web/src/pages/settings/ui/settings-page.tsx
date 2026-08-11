import { Body, Button, Cell, Count, Head, Nothing, Page, Panel, PanelHead, Row, Table } from "@/shared/ui";
import { drop, useResource } from "@/shared/api";
import { money } from "@/shared/lib/format";

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
  const pricing = useResource<{ prices: Price[]; as_of: string }>("/api/pricing");

  const providers = Object.entries(models.data ?? {});
  const prices = pricing.data?.prices ?? [];

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
              {providers.map(([provider, named]) => (
                <Row key={provider}>
                  <td className="text-ink">{provider}</td>
                  <td className="whitespace-normal text-ink-secondary">
                    {named.length > 0 ? named.join(", ") : "none — anything routes through untouched"}
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
          note={pricing.data ? `as of ${pricing.data.as_of}, per million tokens` : undefined}
        />
        {prices.length === 0 ? (
          <Nothing says="No model declares a price. Requests show no cost rather than a cost of zero." />
        ) : (
          <Table>
            <Head>
              <Row>
                <th>Model</th>
                <Count>Input</Count>
                <Count>Output</Count>
                <Count>Cache read</Count>
                <Count>Cache write</Count>
              </Row>
            </Head>
            <Body>
              {prices.map((price) => (
                <Row key={`${price.provider}-${price.model}`}>
                  <td className="text-ink">{price.model}</td>
                  <Cell>{money(price.input_per_mtok_micros)}</Cell>
                  <Cell>{money(price.output_per_mtok_micros)}</Cell>
                  <Cell>{money(price.cache_read_per_mtok_micros)}</Cell>
                  <Cell>{money(price.cache_write_per_mtok_micros)}</Cell>
                </Row>
              ))}
            </Body>
          </Table>
        )}
      </Panel>

      <Panel>
        <PanelHead title="This session" />
        <div className="flex items-center justify-between gap-4 p-3">
          <p className="m-0 text-small text-ink-secondary">
            Signing out clears the cookie in this browser. The admin key itself does not change.
          </p>
          <Button onClick={() => void drop("/api/session").finally(() => window.location.reload())}>
            Sign out
          </Button>
        </div>
      </Panel>
    </Page>
  );
}
