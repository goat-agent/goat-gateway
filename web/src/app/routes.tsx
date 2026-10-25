import { Route, Routes } from "react-router-dom";
import { HomePage } from "@/pages/home";
import { UsagePage } from "@/pages/usage";
import { RequestsPage } from "@/pages/requests";
import { AccountsPage } from "@/pages/accounts";
import { KeysPage } from "@/pages/keys";
import { SettingsPage } from "@/pages/settings";
import { PeoplePage } from "@/pages/people";
import { ProvidersPage } from "@/pages/providers";

export function AppRoutes() {
  return (
    <Routes>
      <Route path="/" element={<HomePage />} />
      <Route path="/usage" element={<UsagePage />} />
      <Route path="/requests/:id?" element={<RequestsPage />} />
      <Route path="/accounts/:id?" element={<AccountsPage />} />
      <Route path="/keys/:id?" element={<KeysPage />} />
      <Route path="/providers/:id?" element={<ProvidersPage />} />
      <Route path="/people/:id?" element={<PeoplePage />} />
      <Route path="/settings" element={<SettingsPage />} />
    </Routes>
  );
}
