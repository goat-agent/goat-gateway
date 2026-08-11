import { useCallback, useEffect } from "react";
import { BrowserRouter } from "react-router-dom";
import { useHappenings, whenLocked } from "@/shared/api";
import { useLive } from "@/entities/request";
import { useLock, Unlock } from "@/features/unlock";
import { useSkin } from "@/features/toggle-theme";
import { AppShell } from "@/widgets/app-shell";
import { Routes } from "./routes";

export function App() {
  const { locked, lock } = useLock();
  const theme = useSkin((held) => held.theme);
  const { opened, settled } = useLive();

  whenLocked(lock);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  useHappenings(
    useCallback(
      (happening) => {
        if (happening.happened === "request_opened") opened();
        if (happening.happened === "request_settled") settled();
      },
      [opened, settled],
    ),
  );

  if (locked) return <Unlock />;

  return (
    <BrowserRouter>
      <AppShell>
        <Routes />
      </AppShell>
    </BrowserRouter>
  );
}
