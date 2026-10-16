import { useCallback, useState } from "react";
import { FilterX, Rows3, Search } from "lucide-react";
import { Button, Input, Nothing, Page, Select } from "@/shared/ui";
import { query, useHappenings, useResource } from "@/shared/api";
import { useCommand } from "@/shared/model/commands";
import { STATUSES, type Request } from "@/entities/request";
import { RequestTable } from "@/widgets/request-table";
import { FilterChips, useFilter } from "@/features/filter-requests";

const FIELDS = ["search", "status", "provider", "account", "model", "person", "client"] as const;

const STATUS_CHOICES = [
  { value: "", label: "Any status" },
  ...STATUSES.map((status) => ({ value: status, label: status.replace("_", " ") })),
];

export function RequestsPage() {
  const { held, set, clear } = useFilter(FIELDS);
  const [typed, setTyped] = useState(held["search"] ?? "");

  const page = useResource<{ requests: Request[]; next_before: number | null }>(
    `/api/requests${query({ ...held, limit: 100 })}`,
  );

  const reload = page.reload;
  useHappenings(
    useCallback(
      (happening) => {
        if (happening === "request_opened" || happening === "request_settled") reload();
      },
      [reload],
    ),
  );

  const requests = page.data?.requests ?? [];
  const narrowed = Object.keys(held).length > 0;
  const elsewhere = Object.fromEntries(
    Object.entries(held).filter(([field]) => field !== "search" && field !== "status"),
  );

  const clearAll = () => {
    setTyped("");
    clear();
  };

  useCommand(
    { id: "requests/clear", label: "Clear the request filters", group: "Requests", icon: FilterX, perform: clearAll },
    narrowed,
  );

  return (
    <Page
      title="Requests"
      note={requests.length > 0 ? `newest first, ${requests.length} shown` : undefined}
      refused={page.error}
      bleed
      toolbar={
        <>
          <form
            onSubmit={(event) => {
              event.preventDefault();
              set("search", typed.trim());
            }}
          >
            <Input
              lead={<Search strokeWidth={1.75} />}
              className="h-7 w-64"
              placeholder="Model, account, request id…"
              aria-label="Search requests"
              value={typed}
              onChange={(event) => setTyped(event.target.value)}
            />
          </form>
          <Select
            label="Status"
            className="w-32"
            value={held["status"] ?? ""}
            choices={STATUS_CHOICES}
            onChange={(value) => set("status", value)}
          />
          <FilterChips held={elsewhere} onDrop={(field) => set(field, "")} />
          {narrowed ? (
            <Button tone="ghost" size="small" className="ml-auto" onClick={clearAll}>
              Clear filters
            </Button>
          ) : null}
        </>
      }
    >
      {requests.length === 0 ? (
        <Nothing
          icon={Rows3}
          says={
            narrowed
              ? "Nothing matches these filters."
              : "No request has reached this gateway yet. Point a client at it and the first one lands here."
          }
          offers={
            narrowed ? (
              <Button size="medium" onClick={clearAll}>
                Clear filters
              </Button>
            ) : undefined
          }
        />
      ) : (
        <RequestTable requests={requests} now={Date.now()} />
      )}
    </Page>
  );
}
