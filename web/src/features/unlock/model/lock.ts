import { create } from "zustand";

type Lock = {
  locked: boolean;
  lock: () => void;
  open: () => void;
};

export const useLock = create<Lock>((set) => ({
  locked: false,
  lock: () => set({ locked: true }),
  open: () => set({ locked: false }),
}));
