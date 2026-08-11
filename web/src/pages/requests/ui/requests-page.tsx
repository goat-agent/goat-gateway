import { useCallback, useState } from "react";
import { Button, Input, Nothing, Page, Panel, PanelHead, Select } from "@/shared/ui";
import { query, useHappenings, useResource } from "@/shared/api";
import { STATUSES, type Request } from "@/entities/request";
import { RequestTable } from "@/widgets/request-table";
import { useFilter } from "@/features/filter-requests";

const FIELDS = ["search", "status", "provider", "account", "model", "person", "client"] as const;

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
          <Select
            className="w-32"
            value={held["status"] ?? ""}
            onChange={(event) => set("status", event.target.value)}
          >
            <option value="">any status</option>
            {STATUSES.map((status) => (
              <option key={status} value={status}>
                {status.replace("_", " ")}
              </option>
            ))}
          </Select>
        </>
      }
    >
      <Panel>
        <PanelHead
          title="Newest first"
          note={requests.length > 0 ? `${requests.length} shown` : undefined}
        />
        {requests.length === 0 ? (
          <Nothing
            says={
              narrowed
                ? "Nothing matches these filters."
                : "No request has reached this gateway yet. Point a client at it and the first one lands here."
            }
            offers={
              narrowed ? (
                <Button
                  onClick={() => {
                    setTyped("");
                    clear();
                  }}
                >
                  Clear filters
                </Button>
              ) : undefined
            }
          />
        ) : (
          <RequestTable requests={requests} now={Date.now()} />
        )}
      </Panel>
    </Page>
  );
}
