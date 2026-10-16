import { Moon, Sun } from "lucide-react";
import { Button, Tooltip } from "@/shared/ui";
import { useTheme } from "../model/store";

export function ThemeButton() {
  const theme = useTheme((store) => store.theme);
  const flip = useTheme((store) => store.flip);
  const said = theme === "dark" ? "Switch to the light theme" : "Switch to the dark theme";

  return (
    <Tooltip label={said} side="top">
      <Button tone="ghost" size="icon" onClick={flip} aria-label={said}>
        {theme === "dark" ? <Sun strokeWidth={1.75} /> : <Moon strokeWidth={1.75} />}
      </Button>
    </Tooltip>
  );
}
