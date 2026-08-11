import { useCallback, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import { Page } from "../page";
import { Panel, PanelHead } from "@/shared/ui/panel";
import { Nothing } from "@/shared/ui/nothing";
import { Button } from "@/shared/ui/button";
import { Input, Select } from "@/shared/ui/field";
import { StatusBadge } from "@/widgets/request-table/status-badge";
import { useResource } from "@/shared/api/use-resource";
import { useHappenings } from "@/shared/api/happenings";
import { query } from "@/shared/api/client";
import type { Request } from "@/entities/types";
import { clock, duration, money, tokens } from "@/shared/lib/format";

const STATUSES = ["", "ok", "error", "in_flight"];

export function RequestsPage() {
  const [params, setParams] = useSearchParams();
  const [typed, setTyped] = useState(params.get("search") ?? "");
  const status = params.get("status") ?? "";
  const provider = params.get("provider") ?? "";

  const page = useResource<{ requests: Request[]; next_before: number | null }>(
    `/api/requests${query({
      search: params.get("search"),
      status: status || undefined,
      provider: provider || undefined,
      limit: 100,
    })}`,
    [params.toString()],
  );

  useHappenings(
    useCallback(
      (happening) => {
        if (happening.happened === "request_opened") page.reload();
      },
      [page],
    ),
  );

  const set = (field: string, value: string) => {
    const next = new URLSearchParams(params);
    if (value) next.set(field, value);
    else next.delete(field);
    setParams(next);
  };

  const rows = page.data?.requests ?? [];
  const filtered = status !== "" || provider !== "" || params.get("search");

  return (
    <Page
      title="Requests"
      note={page.error}
      aside={
        <>
          <form
            onSubmit={(event) => {
              event.preventDefault();
              set("search", typed.trim());
            }}
          >
            <Input
              className="w-56"
              placeholder="model, account, request id…"
              value={typed}
              onChange={(event) => setTyped(event.target.value)}
            />
          </form>
          <Select className="w-32" value={status} onChange={(event) => set("status", event.target.value)}>
            {STATUSES.map((entry) => (
              <option key={entry} value={entry}>
                {entry === "" ? "any status" : entry.replace("_", " ")}
              </option>
            ))}
          </Select>
          <Button tone="quiet" size="small" onClick={() => page.reload()}>
            Refresh
          </Button>
        </>
      }
    >
      <Panel>
        <PanelHead
          title="Newest first"
          note={rows.length > 0 ? `${rows.length} shown` : undefined}
        />
        {rows.length === 0 ? (
          <Nothing
            says={
              filtered
                ? "Nothing matches these filters."
                : "No request has reached this gateway yet. Point a client at it and the first one lands here."
            }
            offers={
              filtered ? (
                <Button onClick={() => setParams(new URLSearchParams())}>Clear filters</Button>
              ) : undefined
            }
          />
        ) : (
          <table className="w-full border-collapse text-small">
            <thead className="[&_th]:h-8 [&_th]:border-b [&_th]:border-line-subtle [&_th]:px-3 [&_th]:text-left [&_th]:text-micro [&_th]:font-medium [&_th]:tracking-wide [&_th]:text-ink-muted [&_th]:uppercase">
              <tr>
                <th>Started</th>
                <th>Status</th>
                <th>Model</th>
                <th>Account</th>
                <th>Client</th>
                <th className="!text-right">In</th>
                <th className="!text-right">Out</th>
                <th className="!text-right">Cost</th>
                <th className="!text-right">Took</th>
              </tr>
            </thead>
            <tbody className="[&_td]:h-9 [&_td]:border-b [&_td]:border-line-subtle [&_td]:px-3 [&_td]:whitespace-nowrap">
              {rows.map((row) => (
                <tr key={row.id} className="hover:bg-raised">
                  <td className="numeric text-ink-secondary">
                    <Link to={`/requests/${row.id}`} className="text-ink-secondary no-underline hover:underline">
                      {clock(row.started_at)}
                    </Link>
                  </td>
                  <td>
                    <StatusBadge status={row.status} kind={row.error_kind} translated={row.translated} />
                  </td>
                  <td className="text-ink">{row.model}</td>
                  <td className="text-ink-secondary">{row.account ?? "—"}</td>
                  <td className="text-ink-secondary">{row.client ?? "—"}</td>
                  <td className="numeric text-right">{tokens(row.usage.input_tokens)}</td>
                  <td className="numeric text-right">{tokens(row.usage.output_tokens)}</td>
                  <td className="numeric text-right">{money(row.cost_micros)}</td>
                  <td className="numeric text-right text-ink-secondary">{duration(row.duration_ms)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Panel>
    </Page>
  );
}
