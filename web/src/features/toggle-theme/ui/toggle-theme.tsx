import { Moon, Sun } from "lucide-react";
import { Button } from "@/shared/ui";
import { useSkin } from "../model/theme";

export function ToggleTheme() {
  const { theme, flip } = useSkin();
  return (
    <Button tone="quiet" size="icon" onClick={flip} aria-label="Switch theme">
      {theme === "dark" ? <Sun /> : <Moon />}
    </Button>
  );
}
