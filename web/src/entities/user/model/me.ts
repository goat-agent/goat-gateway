import { create } from "zustand";
import { ask } from "@/shared/api";
import type { Me } from "./types";

type Held = {
  me: Me | undefined;
  refresh: () => Promise<void>;
  forget: () => void;
};

export const useMe = create<Held>((set) => ({
  me: undefined,
  refresh: () =>
    ask<Me>("/api/me")
      .then((me) => set({ me }))
      .catch(() => set({ me: undefined })),
  forget: () => set({ me: undefined }),
}));
