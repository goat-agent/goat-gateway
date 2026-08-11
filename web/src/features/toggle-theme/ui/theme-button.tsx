import { Moon, Sun } from "lucide-react";
import { Button } from "@/shared/ui";
import { useTheme } from "../model/store";

export function ThemeButton() {
  const theme = useTheme((store) => store.theme);
  const flip = useTheme((store) => store.flip);

  return (
    <Button tone="quiet" size="icon" onClick={flip} aria-label="Switch theme">
      {theme === "dark" ? <Sun /> : <Moon />}
    </Button>
  );
}
