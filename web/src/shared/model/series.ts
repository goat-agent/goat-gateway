import { create } from "zustand";
import { persist } from "zustand/middleware";

const SLOTS = 8;

type Series = {
  slots: Record<string, number>;
  claim: (space: string, key: string) => number;
};

const useSeries = create<Series>()(
  persist(
    (set, get) => ({
      slots: {},
      claim: (space, key) => {
        const held = get().slots[`${space}/${key}`];
        if (held !== undefined) return held;

        const taken = new Set(
          Object.entries(get().slots)
            .filter(([held]) => held.startsWith(`${space}/`))
            .map(([, slot]) => slot),
        );
        let slot = 1;
        while (slot < SLOTS && taken.has(slot)) slot += 1;

        set((held) => ({ slots: { ...held.slots, [`${space}/${key}`]: slot } }));
        return slot;
      },
    }),
    { name: "goat-gateway-series" },
  ),
);

export function seriesColor(space: string, key: string) {
  return `var(--series-${useSeries.getState().claim(space, key)})`;
}
