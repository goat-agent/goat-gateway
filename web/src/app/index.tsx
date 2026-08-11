import { useCallback } from "react";
import { BrowserRouter } from "react-router-dom";
import { AppShell } from "@/widgets/app-shell";
import { Unlock } from "@/features/unlock";
import { useLive } from "@/entities/request";
import { useHappenings } from "@/shared/api";
import { useSession } from "@/shared/model/session";
import { AppRoutes } from "./routes";

export function App() {
  const locked = useSession((store) => store.locked);
  const opened = useLive((store) => store.opened);
  const settled = useLive((store) => store.settled);

  useHappenings(
    useCallback(
      (happening) => {
        if (happening === "request_opened") opened();
        if (happening === "request_settled") settled();
      },
      [opened, settled],
    ),
  );

  if (locked) return <Unlock />;

  return (
    <BrowserRouter>
      <AppShell>
        <AppRoutes />
      </AppShell>
    </BrowserRouter>
  );
}
