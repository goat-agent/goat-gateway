import { useState, type ReactNode } from "react";
import { useSearchParams } from "react-router-dom";
import { Boxes, MoreHorizontal, Pencil, Plus, RotateCcw, Trash2 } from "lucide-react";
import {
  Badge,
  Body,
  Button,
  Count,
  Cell,
  Dialog,
  Head,
  Menu,
  MenuItem,
  MenuSeparator,
  Nothing,
  Page,
  Property,
  Properties,
  Row,
  Table,
  confirmed,
  rowHover,
  toast,
  type Tone,
} from "@/shared/ui";
import { drop, useResource } from "@/shared/api";
import { useCommand } from "@/shared/model/commands";
import { WIRE_LABELS, type ProviderEntry, type Providers } from "@/entities/provider";
import { useMe } from "@/entities/user";
import { DeclareProvider, EditDeclaration } from "@/features/declare-provider";
import { tokens, UNKNOWN } from "@/shared/lib/format";

const LIST =
  "[&_td:first-child]:pl-5 [&_td:last-child]:pr-3 [&_th:first-child]:pl-5 [&_th:last-child]:pr-3 [&_thead_th]:border-t-0";

const ORIGINS: Record<ProviderEntry["origin"], { label: string; tone: Tone }> = {
  built_in: { label: "Built in", tone: "neutral" },
  config: { label: "config.toml", tone: "info" },
  overridden: { label: "Overridden", tone: "warning" },
};

const QUOTA: Record<ProviderEntry["limits"], string> = {
  headers: "From its headers",
  endpoint: "Asked, at most once a minute",
  none: "Not reported",
};

function ways(provider: ProviderEntry) {
  return [
    provider.sign_in ? "plan" : undefined,
    provider.api_key ? "API key" : undefined,
    provider.carries_own_login ? "client's own login" : undefined,
  ].filter((way) => way !== undefined);
}

export function ProvidersPage() {
  const listed = useResource<Providers>("/api/providers");
  const admin = useMe((store) => store.me?.admin ?? false);
  const [asked, setAsked] = useSearchParams();
  const [declaring, setDeclaring] = useState(asked.get("declare") === "1");
  const [editing, setEditing] = useState<ProviderEntry>();
  const [shown, setShown] = useState<ProviderEntry>();

  const writable = admin && (listed.data?.writable ?? false);
  const providers = listed.data?.providers ?? [];

  const startDeclaring = () => {
    setDeclaring(true);
  };
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
      perform: startDeclaring,
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
        setShown(undefined);
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

  const actions = (provider: ProviderEntry) =>
    writable ? (
      <>
        <MenuItem icon={Pencil} onClick={() => setEditing(provider)}>
          {provider.origin === "built_in" ? "Override in config.toml…" : "Edit declaration…"}
        </MenuItem>
        {provider.origin === "built_in" ? null : (
          <>
            <MenuSeparator />
            <MenuItem
              icon={provider.origin === "overridden" ? RotateCcw : Trash2}
              danger={provider.origin === "config"}
              onClick={() => void remove(provider)}
            >
              {provider.origin === "overridden" ? "Restore built-in…" : "Remove…"}
            </MenuItem>
          </>
        )}
      </>
    ) : null;

  return (
    <Page
      title="Providers"
      note="what this gateway knows how to reach"
      refused={listed.error}
      bleed
      aside={
        writable ? (
          <Button tone="primary" onClick={startDeclaring}>
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
      <ProviderDetail provider={shown} onClose={() => setShown(undefined)} actions={actions} />

      {providers.length === 0 && !listed.loading ? (
        <Nothing icon={Boxes} says="No provider is declared. Declare one to start routing to it." />
      ) : (
        <Table contained={false} className={LIST}>
          <Head>
            <Row>
              <th>Provider</th>
              <th className="w-56">Speaks</th>
              <th className="w-56">Served by</th>
              <th className="w-48">Quota</th>
              <Count className="w-20">Models</Count>
              <Count className="w-24">Accounts</Count>
              <th className="w-28">Origin</th>
              <th className="w-12" />
            </Row>
          </Head>
          <Body>
            {providers.map((provider) => (
              <Row
                key={provider.id}
                className={`cursor-default ${rowHover}`}
                onClick={() => setShown(provider)}
              >
                <td>
                  <span className="flex min-w-0 items-baseline gap-2">
                    <span className="truncate font-medium text-ink">{provider.label}</span>
                    <span className="mono truncate text-mini text-ink-muted">{provider.id}</span>
                  </span>
                </td>
                <td className="truncate text-ink-secondary">
                  {provider.endpoints.map((endpoint) => WIRE_LABELS[endpoint.wire]).join(", ")}
                </td>
                <td className="truncate text-ink-secondary">{ways(provider).join(", ") || UNKNOWN}</td>
                <td className="truncate text-ink-secondary">{QUOTA[provider.limits]}</td>
                <Cell className="text-ink-secondary">{provider.models.length || UNKNOWN}</Cell>
                <Cell className={provider.accounts > 0 ? "text-ink" : "text-ink-muted"}>
                  {provider.accounts}
                </Cell>
                <td>
                  <Badge tone={ORIGINS[provider.origin].tone}>{ORIGINS[provider.origin].label}</Badge>
                </td>
                <td className="text-right" onClick={(event) => event.stopPropagation()}>
                  {writable ? (
                    <Menu
                      trigger={
                        <Button tone="ghost" size="icon" aria-label={`Actions for ${provider.label}`}>
                          <MoreHorizontal strokeWidth={1.75} />
                        </Button>
                      }
                    >
                      {actions(provider)}
                    </Menu>
                  ) : null}
                </td>
              </Row>
            ))}
          </Body>
        </Table>
      )}
    </Page>
  );
}

function ProviderDetail({
  provider,
  onClose,
  actions,
}: {
  provider: ProviderEntry | undefined;
  onClose: () => void;
  actions: (provider: ProviderEntry) => ReactNode;
}) {
  const offered = provider ? actions(provider) : null;

  return (
    <Dialog
      open={provider !== undefined}
      onClose={onClose}
      width="wide"
      title={provider?.label ?? ""}
      description={provider ? <span className="mono">{provider.id}</span> : undefined}
    >
      {provider ? (
        <div className="flex flex-col gap-4">
          <Properties>
            <Property name="Key goes in" value={provider.key === "x_api_key" ? "x-api-key" : "Authorization: Bearer"} />
            <Property name="Quota" value={QUOTA[provider.limits]} />
            <Property name="Served by" value={ways(provider).join(", ") || UNKNOWN} />
            {provider.sign_in ? (
              <Property name="Sign-in" value={`${provider.sign_in.label} · ${provider.sign_in.modes.join(", ")}`} />
            ) : null}
            <Property name="Origin" value={ORIGINS[provider.origin].label} />
          </Properties>

          <section className="flex flex-col gap-1.5">
            <h3 className="m-0 text-mini font-medium text-ink-secondary">Endpoints</h3>
            <ul className="m-0 flex list-none flex-col rounded-lg hairline border-line bg-base p-0 py-1">
              {provider.endpoints.map((endpoint) => (
                <li key={endpoint.wire} className="flex min-w-0 items-center gap-3 px-3 py-1 text-small">
                  <span className="w-32 shrink-0 font-medium text-ink">{WIRE_LABELS[endpoint.wire]}</span>
                  <span className="mono min-w-0 truncate text-mini text-ink-secondary">{endpoint.url}</span>
                </li>
              ))}
            </ul>
          </section>

          <section className="flex flex-col gap-1.5">
            <h3 className="m-0 text-mini font-medium text-ink-secondary">
              Models <span className="numeric ml-1">{provider.models.length}</span>
            </h3>
            {provider.models.length === 0 ? (
              <p className="m-0 text-small text-ink-secondary">
                None declared. A model nobody declares still reaches it when it is the only provider
                that speaks the arriving format.
              </p>
            ) : (
              <div className="max-h-72 overflow-y-auto rounded-lg hairline border-line">
                <Table>
                  <Head>
                    <Row>
                      <th>Name</th>
                      <th className="w-28">Thinking</th>
                      <Count className="w-28">Max output</Count>
                      <th className="w-20">Price</th>
                    </Row>
                  </Head>
                  <Body>
                    {provider.models.map((model) => (
                      <Row key={model.name}>
                        <td className="mono truncate text-mini text-ink">{model.name}</td>
                        <td className="text-ink-secondary">{model.thinking ?? UNKNOWN}</td>
                        <Cell className="text-ink-secondary">
                          {tokens(model.max_tokens)}
                        </Cell>
                        <td className={model.priced ? "text-ink-secondary" : "text-ink-muted"}>
                          {model.priced ? "declared" : "none"}
                        </td>
                      </Row>
                    ))}
                  </Body>
                </Table>
              </div>
            )}
          </section>

          {offered ? (
            <div className="flex justify-end">
              <Menu
                trigger={
                  <Button size="medium">
                    <MoreHorizontal strokeWidth={1.75} />
                    Change
                  </Button>
                }
              >
                {offered}
              </Menu>
            </div>
          ) : null}
        </div>
      ) : null}
    </Dialog>
  );
}
