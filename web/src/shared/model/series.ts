import { create } from "zustand";
import { persist } from "zustand/middleware";

const SLOTS = 8;

type Series = { slots: Record<string, number> };

export const useSeries = create<Series>()(
  persist(() => ({ slots: {} }), { name: "goat-gateway-series" }),
);

export function seriesColor(key: string) {
  const { slots } = useSeries.getState();
  const held = slots[key];
  if (held !== undefined) return paint(held);

  const taken = new Set(Object.values(slots));
  let slot = 1;
  while (slot < SLOTS && taken.has(slot)) slot += 1;

  useSeries.setState({ slots: { ...slots, [key]: slot } });
  return paint(slot);
}

function paint(slot: number) {
  return `var(--series-${slot})`;
}
