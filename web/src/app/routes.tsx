import { Route, Routes } from "react-router-dom";
import { OverviewPage } from "@/pages/overview";
import { UsagePage } from "@/pages/usage";
import { RequestsPage } from "@/pages/requests";
import { RequestPage } from "@/pages/request";
import { AccountsPage } from "@/pages/accounts";
import { KeysPage } from "@/pages/keys";
import { SettingsPage } from "@/pages/settings";
import { PeoplePage } from "@/pages/people";
import { ProvidersPage } from "@/pages/providers";

export function AppRoutes() {
  return (
    <Routes>
      <Route path="/" element={<OverviewPage />} />
      <Route path="/usage" element={<UsagePage />} />
      <Route path="/requests" element={<RequestsPage />} />
      <Route path="/requests/:id" element={<RequestPage />} />
      <Route path="/accounts" element={<AccountsPage />} />
      <Route path="/keys" element={<KeysPage />} />
      <Route path="/providers" element={<ProvidersPage />} />
      <Route path="/people" element={<PeoplePage />} />
      <Route path="/settings" element={<SettingsPage />} />
    </Routes>
  );
}
