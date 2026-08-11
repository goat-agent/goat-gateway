import { create } from "zustand";

type Session = {
  locked: boolean;
  lock: () => void;
  unlock: () => void;
};

export const useSession = create<Session>((set) => ({
  locked: false,
  lock: () => set({ locked: true }),
  unlock: () => set({ locked: false }),
}));
