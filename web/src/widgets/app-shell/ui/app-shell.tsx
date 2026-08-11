import type { ReactNode } from "react";
import { NavLink } from "react-router-dom";
import { Activity, KeyRound, LayoutGrid, ListOrdered, PlugZap, Settings } from "lucide-react";
import { useLive } from "@/entities/request";
import { ToggleTheme } from "@/features/toggle-theme";
import { cn } from "@/shared/lib";

const PLACES = [
  { to: "/", label: "Overview", icon: LayoutGrid, end: true },
  { to: "/usage", label: "Usage", icon: Activity, end: false },
  { to: "/requests", label: "Requests", icon: ListOrdered, end: false },
  { to: "/accounts", label: "Accounts", icon: PlugZap, end: false },
  { to: "/keys", label: "Keys", icon: KeyRound, end: false },
  { to: "/settings", label: "Settings", icon: Settings, end: false },
];

export function AppShell({ children }: { children: ReactNode }) {
  const inFlight = useLive((held) => held.inFlight);

  return (
    <div className="flex h-full">
      <nav className="flex w-[var(--sidebar-width)] shrink-0 flex-col border-r border-line-subtle bg-panel">
        <div className="flex h-12 items-center gap-2 px-3">
          <span className="size-2 rounded-full" style={{ background: "var(--series-3)" }} />
          <span className="text-small font-semibold tracking-tight">goat gateway</span>
        </div>

        <ul className="m-0 flex list-none flex-col gap-0.5 p-2">
          {PLACES.map((place) => (
            <li key={place.to}>
              <NavLink
                to={place.to}
                end={place.end}
                className={({ isActive }) =>
                  cn(
                    "flex h-8 items-center gap-2.5 rounded-sm px-2 text-small no-underline transition-colors",
                    isActive
                      ? "bg-raised text-ink"
                      : "text-ink-secondary hover:bg-raised hover:text-ink",
                  )
                }
              >
                <place.icon className="size-4 shrink-0" strokeWidth={1.75} />
                {place.label}
              </NavLink>
            </li>
          ))}
        </ul>

        <div className="mt-auto flex items-center justify-between gap-2 border-t border-line-subtle px-3 py-2">
          <span className="flex items-center gap-1.5 text-micro text-ink-muted">
            <span
              className={cn("size-1.5 rounded-full", inFlight > 0 && "animate-pulse")}
              style={{ background: inFlight > 0 ? "var(--series-3)" : "var(--line-strong)" }}
            />
            {inFlight > 0 ? `${inFlight} in flight` : "idle"}
          </span>
          <ToggleTheme />
        </div>
      </nav>

      <main className="flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-[var(--content-max)] px-6 py-5">{children}</div>
      </main>
    </div>
  );
}
