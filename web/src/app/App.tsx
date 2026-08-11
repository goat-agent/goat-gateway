import { useCallback, useState } from "react";
import { BrowserRouter, Route, Routes } from "react-router-dom";
import { Sidebar } from "@/widgets/sidebar/sidebar";
import { Unlock } from "@/features/unlock/unlock";
import { whenLocked } from "@/shared/api/client";
import { useHappenings } from "@/shared/api/happenings";
import { OverviewPage } from "@/pages/overview/overview-page";
import { UsagePage } from "@/pages/usage/usage-page";
import { RequestsPage } from "@/pages/requests/requests-page";
import { RequestPage } from "@/pages/request/request-page";
import { AccountsPage } from "@/pages/accounts/accounts-page";
import { KeysPage } from "@/pages/keys/keys-page";
import { SettingsPage } from "@/pages/settings/settings-page";

export function App() {
  const [locked, setLocked] = useState(false);
  const [inFlight, setInFlight] = useState(0);

  whenLocked(useCallback(() => setLocked(true), []));

  useHappenings(
    useCallback((happening) => {
      if (happening.happened === "request_opened") setInFlight((live) => live + 1);
      if (happening.happened === "request_settled") setInFlight((live) => Math.max(0, live - 1));
    }, []),
  );

  if (locked) return <Unlock onOpen={() => setLocked(false)} />;

  return (
    <BrowserRouter>
      <div className="flex h-full">
        <Sidebar inFlight={inFlight} />
        <main className="flex-1 overflow-y-auto">
          <div className="mx-auto w-full max-w-[var(--content-max)] px-6 py-5">
            <Routes>
              <Route path="/" element={<OverviewPage />} />
              <Route path="/usage" element={<UsagePage />} />
              <Route path="/requests" element={<RequestsPage />} />
              <Route path="/requests/:id" element={<RequestPage />} />
              <Route path="/accounts" element={<AccountsPage />} />
              <Route path="/keys" element={<KeysPage />} />
              <Route path="/settings" element={<SettingsPage />} />
            </Routes>
          </div>
        </main>
      </div>
    </BrowserRouter>
  );
}
