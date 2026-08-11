import { create } from "zustand";
import { persist } from "zustand/middleware";

const SLOTS = 8;

type Series = {
  slots: Record<string, number>;
  claim: (key: string) => number;
};

const useSeries = create<Series>()(
  persist(
    (set, get) => ({
      slots: {},
      claim: (key) => {
        const held = get().slots[key];
        if (held !== undefined) return held;

        const taken = new Set(Object.values(get().slots));
        let slot = 1;
        while (slot < SLOTS && taken.has(slot)) slot += 1;

        set((held) => ({ slots: { ...held.slots, [key]: slot } }));
        return slot;
      },
    }),
    { name: "goat-gateway-series" },
  ),
);

export function seriesColor(key: string) {
  return `var(--series-${useSeries.getState().claim(key)})`;
}
