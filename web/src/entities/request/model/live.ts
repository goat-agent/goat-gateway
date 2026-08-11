import { create } from "zustand";

type Live = {
  inFlight: number;
  opened: () => void;
  settled: () => void;
};

export const useLive = create<Live>((set) => ({
  inFlight: 0,
  opened: () => set((held) => ({ inFlight: held.inFlight + 1 })),
  settled: () => set((held) => ({ inFlight: Math.max(0, held.inFlight - 1) })),
}));
