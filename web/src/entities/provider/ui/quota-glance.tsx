import { Meter } from "@/shared/ui";
import { within } from "@/shared/lib/format";
import { worstWindow } from "../model/types";
import type { Quota } from "../model/quota";

export function QuotaGlance({
  quota,
  account,
  now,
  resets = false,
}: {
  quota: Quota | undefined;
  account: string;
  now: number;
  resets?: boolean;
}) {
  if (!quota) return null;
  if (quota.seen === "never") return <span className="text-mini text-ink-muted">not reported</span>;
  if (quota.seen === "not_yet") {
    return <span className="text-mini text-ink-secondary">not observed yet</span>;
  }
  if (quota.seen === "unread") {
    return <span className="text-mini text-warning-text">reported, not understood</span>;
  }

  const worst = worstWindow(quota.windows);
  if (!worst) return null;
  return (
    <span className="flex min-w-0 items-center gap-2.5">
      <Meter
        used={worst.used_percent}
        label={`${worst.label} used on ${account}`}
        className="min-w-0 flex-1"
      />
      <span className="w-14 shrink-0 truncate text-mini text-ink-secondary">
        {resets && worst.resets_at_ms ? within(worst.resets_at_ms, now) : worst.label}
      </span>
    </span>
  );
}
