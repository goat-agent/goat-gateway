import { Badge, type Tone } from "@/shared/ui";
import { within } from "@/shared/lib/format";
import type { AccountState } from "../model/types";

const SAID: Record<AccountState, { text: string; tone: Tone }> = {
  active: { text: "usable", tone: "good" },
  rate_limited: { text: "rate limited", tone: "warning" },
  sign_in_expired: { text: "signed out", tone: "serious" },
  disabled: { text: "turned off", tone: "neutral" },
};

export function StateBadge({
  state,
  until,
  now,
}: {
  state: AccountState;
  until: number | null;
  now: number;
}) {
  const said = SAID[state];
  return (
    <Badge tone={said.tone}>
      {said.text}
      {state === "rate_limited" && until ? ` · ${within(until, now)}` : ""}
    </Badge>
  );
}
