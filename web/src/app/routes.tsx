import { Route, Routes as Switch } from "react-router-dom";
import { AccountsPage } from "@/pages/accounts";
import { KeysPage } from "@/pages/keys";
import { OverviewPage } from "@/pages/overview";
import { RequestPage } from "@/pages/request";
import { RequestsPage } from "@/pages/requests";
import { SettingsPage } from "@/pages/settings";
import { UsagePage } from "@/pages/usage";

export function Routes() {
  return (
    <Switch>
      <Route path="/" element={<OverviewPage />} />
      <Route path="/usage" element={<UsagePage />} />
      <Route path="/requests" element={<RequestsPage />} />
      <Route path="/requests/:id" element={<RequestPage />} />
      <Route path="/accounts" element={<AccountsPage />} />
      <Route path="/keys" element={<KeysPage />} />
      <Route path="/settings" element={<SettingsPage />} />
    </Switch>
  );
}
