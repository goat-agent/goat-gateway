import { useCallback, useEffect, useState } from "react";

type Theme = "dark" | "light";
const REMEMBERED = "goat-gateway-theme";

export function useTheme() {
  const [theme, setTheme] = useState<Theme>(
    () => (localStorage.getItem(REMEMBERED) as Theme | null) ?? "dark",
  );

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    localStorage.setItem(REMEMBERED, theme);
  }, [theme]);

  const flip = useCallback(
    () => setTheme((held) => (held === "dark" ? "light" : "dark")),
    [],
  );

  return { theme, flip };
}
