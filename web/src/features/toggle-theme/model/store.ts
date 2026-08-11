import { create } from "zustand";
import { persist } from "zustand/middleware";

export type Theme = "dark" | "light";

type ThemeStore = {
  theme: Theme;
  flip: () => void;
};

export const useTheme = create<ThemeStore>()(
  persist(
    (set) => ({
      theme: "dark",
      flip: () => set((held) => ({ theme: held.theme === "dark" ? "light" : "dark" })),
    }),
    { name: "goat-gateway-theme" },
  ),
);

function wear(theme: Theme) {
  document.documentElement.dataset["theme"] = theme;
}

useTheme.subscribe((store) => wear(store.theme));
wear(useTheme.getState().theme);
