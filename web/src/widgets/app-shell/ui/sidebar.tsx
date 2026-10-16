import { Keyboard } from "lucide-react";
import { NavLink } from "react-router-dom";
import { Button, Mark, Tooltip } from "@/shared/ui";
import { cn } from "@/shared/lib/cn";
import { ThemeButton } from "@/features/toggle-theme";
import { useLive } from "@/entities/request";
import { PLACES, SECTIONS } from "../model/places";
import { useSheet } from "../model/sheet";

export function Sidebar() {
  return (
    <nav
      aria-label="Places"
      className="flex w-[var(--sidebar-width)] shrink-0 flex-col bg-base select-none"
    >
      <div className="flex h-[var(--bar-height)] items-center gap-2 px-[15px] pt-1">
        <Mark />
        <span className="truncate text-small font-semibold text-ink-title">goat gateway</span>
      </div>

      <div className="flex flex-col gap-4 overflow-y-auto px-2 pt-2">
        {SECTIONS.map((section) => (
          <div key={section} className="flex flex-col gap-0.5">
            <span className="flex h-7 items-center px-[7px] text-mini font-medium text-ink-secondary">
              {section}
            </span>
            {PLACES.filter((place) => place.section === section).map((place) => (
              <NavLink
                key={place.to}
                to={place.to}
                end={place.end}
                className={({ isActive }) =>
                  cn(
                    "hover-fade group flex h-[var(--sidebar-item)] items-center gap-2 rounded-lg px-[7px] text-small font-medium",
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
                <span className="truncate">{place.label}</span>
              </NavLink>
            ))}
          </div>
        ))}
      </div>

      <div className="mt-auto flex items-center justify-between gap-2 py-2.5 pr-2 pl-[15px]">
        <InFlight />
        <span className="flex items-center gap-0.5">
          <Tooltip label="Keyboard shortcuts" keys={["?"]} side="top">
            <Button
              tone="ghost"
              size="icon"
              aria-label="Keyboard shortcuts"
              onClick={() => useSheet.getState().setOpen(true)}
            >
              <Keyboard strokeWidth={1.75} />
            </Button>
          </Tooltip>
          <ThemeButton />
        </span>
      </div>
    </nav>
  );
}

function InFlight() {
  const inFlight = useLive((store) => store.inFlight);
  const busy = inFlight > 0;

  return (
    <span
      className="flex min-w-0 items-center gap-2 text-mini text-ink-secondary"
      aria-live="polite"
    >
      <span className="relative flex size-2 shrink-0">
        {busy ? (
          <span className="absolute inset-0 animate-ping rounded-full bg-good opacity-60" />
        ) : null}
        <span className={cn("relative size-2 rounded-full", busy ? "bg-good" : "bg-line-strong")} />
      </span>
      <span className="numeric truncate">{busy ? `${inFlight} in flight` : "Idle"}</span>
    </span>
  );
}
