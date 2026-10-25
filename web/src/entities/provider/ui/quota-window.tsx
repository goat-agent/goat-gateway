import { Meter } from "@/shared/ui";
import { within } from "@/shared/lib/format";
import type { LimitWindow } from "../model/types";

export function QuotaWindow({
  window,
  account,
  now,
}: {
  window: LimitWindow;
  account: string;
  now: number;
}) {
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <div className="flex min-w-0 items-baseline justify-between gap-3 text-small">
        <span className="min-w-0 truncate text-ink">
          {window.label}
          <span className="ml-1.5 text-ink-secondary">{window.scope ?? "every model"}</span>
        </span>
        <span className="numeric shrink-0 text-mini text-ink-secondary">
          {window.resets_at_ms ? `resets in ${within(window.resets_at_ms, now)}` : "reset not reported"}
        </span>
      </div>
      <Meter used={window.used_percent} label={`${window.label} used on ${account}`} />
    </div>
  );
}
