import { Badge, Dot, type DotTone, type Tone } from "@/shared/ui";
import { within } from "@/shared/lib/format";
import type { AccountState } from "../model/types";

const SAID: Record<AccountState, { text: string; tone: Tone; dot: DotTone }> = {
  active: { text: "usable", tone: "good", dot: "good" },
  rate_limited: { text: "rate limited", tone: "warning", dot: "warning" },
  sign_in_expired: { text: "signed out", tone: "serious", dot: "serious" },
  disabled: { text: "turned off", tone: "neutral", dot: "off" },
};

export function stateWords(state: AccountState, until: number | null, now: number) {
  const text = SAID[state].text;
  return state === "rate_limited" && until ? `${text} for ${within(until, now)}` : text;
}

export function StateDot({
  state,
  until,
  now,
}: {
  state: AccountState;
  until: number | null;
  now: number;
}) {
  return <Dot tone={SAID[state].dot} label={stateWords(state, until, now)} />;
}

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
    <Badge dot tone={said.tone}>
      {said.text}
      {state === "rate_limited" && until ? (
        <span className="numeric opacity-80">· {within(until, now)}</span>
      ) : null}
    </Badge>
  );
}
