import { useState } from "react";
import { useSearchParams } from "react-router-dom";
import { Boxes, Plus } from "lucide-react";
import {
  Badge,
  Body,
  Button,
  Cell,
  Count,
  Head,
  Nothing,
  Page,
  Row,
  Split,
  Table,
  confirmed,
  rowHover,
  toast,
} from "@/shared/ui";
import { drop, useResource } from "@/shared/api";
import { useCommand } from "@/shared/model/commands";
import { useRowKeys } from "@/shared/lib/use-row-keys";
import { useSelection } from "@/shared/lib/use-selection";
import { WIRE_LABELS, type ProviderEntry, type Providers } from "@/entities/provider";
import { useMe } from "@/entities/user";
import { DeclareProvider, EditDeclaration } from "@/features/declare-provider";
import { UNKNOWN } from "@/shared/lib/format";
import { ORIGINS, ProviderInspector, ways, type Pricing } from "./provider-inspector";

export function ProvidersPage() {
  const listed = useResource<Providers>("/api/providers");
  const pricing = useResource<Pricing>("/api/pricing");
  const admin = useMe((store) => store.me?.admin ?? false);
  const [asked, setAsked] = useSearchParams();
  const [declaring, setDeclaring] = useState(asked.get("declare") === "1");
  const [editing, setEditing] = useState<ProviderEntry>();
  const { selected, select, close } = useSelection("/providers");

  const writable = admin && (listed.data?.writable ?? false);
  const providers = listed.data?.providers ?? [];
  const shown = providers.find((provider) => provider.id === selected);
  useRowKeys({ ids: providers.map((provider) => provider.id), selected, select, close });

  const stopDeclaring = () => {
    setDeclaring(false);
    if (asked.has("declare")) setAsked({}, { replace: true });
  };

  useCommand(
    {
      id: "providers/declare",
      label: "Declare a provider",
      group: "Providers",
      icon: Plus,
      perform: () => setDeclaring(true),
    },
    writable,
  );

  const remove = async (provider: ProviderEntry) => {
    const overridden = provider.origin === "overridden";
    const yes = await confirmed({
      title: overridden ? `Restore the built-in ${provider.label}?` : `Remove ${provider.label}?`,
      says: overridden
        ? "Your declaration is taken out of config.toml and the built-in one serves again."
        : provider.accounts > 0
          ? `Its declaration is taken out of config.toml. Its ${provider.accounts} account${provider.accounts === 1 ? "" : "s"} stay registered but nothing routes to them until it is declared again.`
          : "Its declaration is taken out of config.toml.",
      action: overridden ? "Restore built-in" : "Remove provider",
    });
    if (!yes) return;
    drop(`/api/providers/${encodeURIComponent(provider.id)}`)
      .then(() => {
        toast(overridden ? `${provider.label} is the built-in one again` : `Removed ${provider.label}`);
        if (!overridden) close();
        listed.reload();
      })
      .catch((error: Error) =>
        toast(`Could not change ${provider.label}`, { says: error.message, tone: "critical" }),
      );
  };

  const saved = (id: string) => {
    toast(`Saved ${id}`, { says: "The gateway reloaded it. No restart needed.", tone: "good" });
    listed.reload();
  };

  return (
    <Page
      title="Providers"
      count={listed.data ? providers.length : undefined}
      refused={listed.error}
      scroll={false}
      actions={
        writable ? (
          <Button tone="primary" onClick={() => setDeclaring(true)}>
            <Plus strokeWidth={2} />
            Declare a provider
          </Button>
        ) : null
      }
    >
      <DeclareProvider open={declaring && writable} onClose={stopDeclaring} onSaved={saved} />
      <EditDeclaration
        id={editing?.id}
        label={editing?.label ?? ""}
        onClose={() => setEditing(undefined)}
        onSaved={saved}
      />

      <Split
        label="Provider"
        list={
          providers.length === 0 ? (
            listed.data ? (
              <Nothing icon={Boxes} says="No provider is declared. Declare one to start routing to it." />
            ) : null
          ) : (
            <Table sticky className="table-fixed">
              <Head>
                <Row>
                  <th>Provider</th>
                  <th className="hidden w-56 @3xl:table-cell">Speaks</th>
                  <th className="hidden w-52 @4xl:table-cell">Served by</th>
                  <Count className="w-24">Accounts</Count>
                  <th className="w-32">Origin</th>
                </Row>
              </Head>
              <Body>
                {providers.map((provider) => (
                  <Row
                    key={provider.id}
                    data-row={provider.id}
                    selected={provider.id === selected}
                    className={rowHover}
                    onClick={() => select(provider.id)}
                  >
                    <td className="truncate">
                      <span className="font-medium text-ink">{provider.label}</span>
                      <span className="mono ml-2 text-ink-muted">{provider.id}</span>
                    </td>
                    <td className="hidden truncate text-ink-secondary @3xl:table-cell">
                      {provider.endpoints.map((endpoint) => WIRE_LABELS[endpoint.wire]).join(", ")}
                    </td>
                    <td className="hidden truncate text-ink-secondary @4xl:table-cell">
                      {ways(provider).join(", ") || UNKNOWN}
                    </td>
                    <Cell className={provider.accounts > 0 ? "text-ink" : "text-ink-muted"}>
                      {provider.accounts}
                    </Cell>
                    <td>
                      <Badge tone={ORIGINS[provider.origin].tone}>{ORIGINS[provider.origin].label}</Badge>
                    </td>
                  </Row>
                ))}
              </Body>
            </Table>
          )
        }
        inspector={
          selected ? (
            <ProviderInspector
              id={selected}
              provider={shown}
              loaded={listed.data !== undefined}
              pricing={pricing.data}
              writable={writable}
              onEdit={setEditing}
              onRemove={(provider) => void remove(provider)}
              onClose={close}
            />
          ) : undefined
        }
      />
    </Page>
  );
}
