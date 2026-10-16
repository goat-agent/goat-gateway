import { useCallback, useEffect } from "react";
import { BrowserRouter, Route, Routes } from "react-router-dom";
import { AppShell } from "@/widgets/app-shell";
import { Unlock } from "@/features/unlock";
import { InvitePage } from "@/pages/invite";
import { useLive } from "@/entities/request";
import { useMe } from "@/entities/user";
import { useHappenings } from "@/shared/api";
import { useSession } from "@/shared/model/session";
import { ConfirmationHost, ToastHost, TooltipProvider } from "@/shared/ui";
import { AppRoutes } from "./routes";

export function App() {
  return (
    <ToastHost>
      <TooltipProvider>
        <BrowserRouter>
          <Routes>
            <Route path="/invite/:token" element={<InvitePage />} />
            <Route path="*" element={<Console />} />
          </Routes>
        </BrowserRouter>
        <ConfirmationHost />
      </TooltipProvider>
    </ToastHost>
  );
}

function Console() {
  const locked = useSession((store) => store.locked);
  const refresh = useMe((store) => store.refresh);

  useEffect(() => {
    if (!locked) void refresh();
  }, [locked, refresh]);

  if (locked) return <Unlock />;
  return (
    <Live>
      <AppShell>
        <AppRoutes />
      </AppShell>
    </Live>
  );
}

function Live({ children }: { children: React.ReactNode }) {
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

  return children;
}
