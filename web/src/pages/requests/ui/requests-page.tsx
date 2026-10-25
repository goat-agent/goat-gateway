import { useCallback, useState } from "react";
import { FilterX, Rows3, Search } from "lucide-react";
import { Button, Input, Nothing, Page, Select, Split } from "@/shared/ui";
import { query, useHappenings, useResource } from "@/shared/api";
import { useCommand } from "@/shared/model/commands";
import { useRowKeys } from "@/shared/lib/use-row-keys";
import { useSelection } from "@/shared/lib/use-selection";
import { STATUSES, type Request } from "@/entities/request";
import { RequestTable } from "@/widgets/request-table";
import { FilterChips, useFilter } from "@/features/filter-requests";
import { RequestInspector } from "./request-inspector";

const FIELDS = ["search", "status", "provider", "account", "model", "person", "key"] as const;

const STATUS_CHOICES = [
  { value: "", label: "Any status" },
  ...STATUSES.map((status) => ({ value: status, label: status.replace("_", " ") })),
];

export function RequestsPage() {
  const { held, set, clear } = useFilter(FIELDS);
  const [typed, setTyped] = useState(held["search"] ?? "");
  const { selected, select, close } = useSelection("/requests");

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
  useRowKeys({ ids: requests.map((request) => request.id), selected, select, close });

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
      count={page.data ? requests.length : undefined}
      refused={page.error}
      scroll={false}
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
      <Split
        label="Request"
        list={
          requests.length === 0 ? (
            page.data ? (
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
            ) : null
          ) : (
            <RequestTable requests={requests} now={Date.now()} selected={selected} onSelect={select} />
          )
        }
        inspector={selected ? <RequestInspector id={selected} onClose={close} /> : undefined}
      />
    </Page>
  );
}
