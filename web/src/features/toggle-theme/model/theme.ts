import { create } from "zustand";
import { persist } from "zustand/middleware";

type Theme = "dark" | "light";

type Skin = {
  theme: Theme;
  flip: () => void;
};

export const useSkin = create<Skin>()(
  persist(
    (set) => ({
      theme: "dark",
      flip: () => set((held) => ({ theme: held.theme === "dark" ? "light" : "dark" })),
    }),
    { name: "goat-gateway-theme" },
  ),
);
