import { useCallback } from "react";
import { useSearchParams } from "react-router-dom";
import { query, useHappenings, useResource } from "@/shared/api";
import { Button, Nothing, Page, Panel, PanelHead } from "@/shared/ui";
import type { Request } from "@/entities/request";
import { FilterRequests } from "@/features/filter-requests";
import { RequestList } from "@/widgets/request-list";

type Found = { requests: Request[]; next_before: number | null };

export function RequestsPage() {
  const [params, setParams] = useSearchParams();
  const asked = `/api/requests${query({
    search: params.get("search"),
    status: params.get("status"),
    provider: params.get("provider"),
    account: params.get("account"),
    conversation: params.get("conversation"),
    limit: 200,
  })}`;

  const page = useResource<Found>(asked, [asked]);

  useHappenings(
    useCallback(
      (happening) => {
        if (happening.happened === "request_opened") page.reload();
      },
      [page],
    ),
  );

  const rows = page.data?.requests ?? [];
  const narrowed = [...params.keys()].length > 0;

  return (
    <Page
      title="Requests"
      note={page.error}
      aside={<FilterRequests params={params} onChange={setParams} />}
    >
      <Panel>
        <PanelHead title="Newest first" note={rows.length > 0 ? `${rows.length} shown` : undefined} />
        {rows.length === 0 ? (
          <Nothing
            says={
              narrowed
                ? "Nothing matches these filters."
                : "No request has reached this gateway yet. Point a client at it and the first one lands here."
            }
            offers={
              narrowed ? (
                <Button onClick={() => setParams(new URLSearchParams())}>Clear filters</Button>
              ) : undefined
            }
          />
        ) : (
          <RequestList requests={rows} />
        )}
      </Panel>
    </Page>
  );
}
