import { useCallback } from "react";
import { ChevronDown, Keyboard, LogOut, Moon, Search, Settings, Sun } from "lucide-react";
import { NavLink, useNavigate } from "react-router-dom";
import { Dot, Keys, Mark, Menu, MenuItem, MenuLabel, MenuSeparator } from "@/shared/ui";
import { useHappenings, useResource } from "@/shared/api";
import { useCommands } from "@/shared/model/commands";
import { cn } from "@/shared/lib/cn";
import { MOD } from "@/shared/lib/keys";
import { useTheme } from "@/features/toggle-theme";
import { signOut } from "@/features/sign-out";
import { useLive } from "@/entities/request";
import { useMe } from "@/entities/user";
import type { Account } from "@/entities/account";
import { SECTIONS, usePlaces, type Place } from "../model/places";
import { useSheet } from "../model/sheet";

const item =
  "hover-fade group flex h-[var(--sidebar-item)] w-full items-center gap-2 rounded-lg px-[7px] text-small font-medium outline-none";

export function Sidebar() {
  const places = usePlaces();
  const inFlight = useLive((store) => store.inFlight);
  const stuck = useStuckAccounts();

  const badgeOf = (place: Place) => {
    if (place.to === "/requests" && inFlight > 0) {
      return <span className="numeric text-mini text-good-text">{inFlight}</span>;
    }
    if (place.to === "/accounts" && stuck > 0) {
      return (
        <span className="flex items-center gap-1.5 text-mini text-warning-text">
          <Dot tone="warning" label={`${stuck} accounts cannot serve right now`} />
          <span className="numeric">{stuck}</span>
        </span>
      );
    }
    return null;
  };

  return (
    <nav
      aria-label="Places"
      className="flex w-[var(--sidebar-width)] shrink-0 flex-col gap-2 bg-base px-2 select-none"
    >
      <div className="flex h-[var(--bar-height)] shrink-0 items-center pt-2">
        <Workspace />
      </div>

      <button
        type="button"
        onClick={() => useCommands.getState().show()}
        className={cn(
          item,
          "hairline border-line bg-panel text-ink-secondary shadow-low hover:bg-nav-hover hover:text-ink",
        )}
      >
        <Search className="size-4 shrink-0" strokeWidth={1.75} />
        <span className="grow text-left">Search</span>
        <Keys keys={[MOD, "K"]} />
      </button>

      <div className="flex min-h-0 flex-col gap-4 overflow-y-auto pt-2">
        {SECTIONS.map((section, index) => (
          <div key={section} className="flex flex-col gap-px">
            {index > 0 ? (
              <span className="flex h-7 items-center px-[7px] text-mini font-medium text-ink-muted">
                {section}
              </span>
            ) : null}
            {places
              .filter((place) => place.section === section)
              .map((place) => (
                <NavLink
                  key={place.to}
                  to={place.to}
                  end={place.end}
                  className={({ isActive }) =>
                    cn(
                      item,
                      isActive
                        ? "bg-nav-active text-ink-title"
                        : "text-ink hover:bg-nav-hover hover:text-ink-title",
                    )
                  }
                >
                  <place.icon
                    className="size-4 shrink-0 text-ink-secondary group-aria-[current=page]:text-ink-title"
                    strokeWidth={1.75}
                  />
                  <span className="grow truncate">{place.label}</span>
                  {badgeOf(place)}
                </NavLink>
              ))}
          </div>
        ))}
      </div>

      <div className="mt-auto flex h-10 shrink-0 items-center px-[7px] pb-1">
        <InFlight count={inFlight} />
      </div>
    </nav>
  );
}

function Workspace() {
  const navigate = useNavigate();
  const me = useMe((store) => store.me);
  const theme = useTheme((store) => store.theme);
  const flip = useTheme((store) => store.flip);
  const person = me?.person ?? null;

  return (
    <Menu
      align="start"
      trigger={
        <button
          type="button"
          className={cn(item, "w-auto max-w-full text-ink-title hover:bg-nav-hover data-[popup-open]:bg-nav-hover")}
        >
          <Mark />
          <span className="truncate font-semibold">goat gateway</span>
          <ChevronDown className="size-3.5 shrink-0 text-ink-muted" strokeWidth={1.75} />
        </button>
      }
    >
      <MenuLabel>
        {person ? `${person.name} · ${person.role}` : "Signed in with the admin key"}
      </MenuLabel>
      <MenuItem icon={Settings} keys={["G", "S"]} onClick={() => navigate("/settings")}>
        Settings
      </MenuItem>
      <MenuItem icon={theme === "dark" ? Sun : Moon} onClick={flip}>
        {theme === "dark" ? "Light theme" : "Dark theme"}
      </MenuItem>
      <MenuItem icon={Keyboard} keys={["?"]} onClick={() => useSheet.getState().setOpen(true)}>
        Keyboard shortcuts
      </MenuItem>
      <MenuSeparator />
      <MenuItem icon={LogOut} onClick={signOut}>
        Sign out
      </MenuItem>
    </Menu>
  );
}

function InFlight({ count }: { count: number }) {
  const busy = count > 0;
  return (
    <span className="flex min-w-0 items-center gap-2 text-mini text-ink-secondary" aria-live="polite">
      <Dot tone={busy ? "good" : "neutral"} pulse={busy} />
      <span className="numeric truncate">{busy ? `${count} in flight` : "Idle"}</span>
    </span>
  );
}

function useStuckAccounts() {
  const accounts = useResource<{ accounts: Account[] }>("/api/accounts");
  const reload = accounts.reload;
  useHappenings(
    useCallback(
      (happening) => {
        if (happening === "account_changed" || happening === "limits_observed") reload();
      },
      [reload],
    ),
  );
  return (accounts.data?.accounts ?? []).filter(
    (account) => account.state === "rate_limited" || account.state === "sign_in_expired",
  ).length;
}
