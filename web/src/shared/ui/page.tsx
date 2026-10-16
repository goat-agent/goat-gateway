import { ChevronRight, CircleAlert, Search } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "@/shared/lib/cn";
import { MOD } from "@/shared/lib/keys";
import { useCommands } from "@/shared/model/commands";
import { Button } from "./button";
import { Keys } from "./kbd";
import { Tooltip } from "./tooltip";

export function Page({
  title,
  trail,
  note,
  aside,
  toolbar,
  refused,
  bleed = false,
  children,
}: {
  title: ReactNode;
  trail?: ReactNode;
  note?: ReactNode;
  aside?: ReactNode;
  toolbar?: ReactNode;
  refused?: string;
  bleed?: boolean;
  children: ReactNode;
}) {
  return (
    <div data-slot="page" className="flex h-full min-h-0 flex-col">
      <header className="flex h-[var(--bar-height)] shrink-0 items-center gap-2 hairline-b border-line-subtle pr-2.5 pl-5 select-none">
        {trail ? (
          <>
            <span className="shrink-0 text-small font-medium text-ink-secondary">{trail}</span>
            <ChevronRight className="size-3.5 shrink-0 text-ink-muted" strokeWidth={1.75} />
          </>
        ) : null}
        <h1 className="m-0 min-w-0 truncate text-small font-medium text-ink-title">{title}</h1>
        {note ? <span className="min-w-0 truncate text-small text-ink-secondary">{note}</span> : null}
        <div className="ml-auto flex shrink-0 items-center gap-1.5">
          {aside}
          <CommandButton />
        </div>
      </header>

      {toolbar ? (
        <div className="flex min-h-[var(--bar-height)] shrink-0 flex-wrap items-center gap-2 hairline-b border-line-subtle px-5 py-2 select-none">
          {toolbar}
        </div>
      ) : null}

      {refused ? (
        <div className="flex shrink-0 items-center gap-2 hairline-b border-line-subtle bg-critical-tint px-5 py-2 text-small text-critical-text">
          <CircleAlert className="size-4 shrink-0" strokeWidth={1.75} />
          {refused}
        </div>
      ) : null}

      <div className="min-h-0 flex-1 overflow-y-auto">
        {bleed ? (
          children
        ) : (
          <div className="mx-auto flex w-full max-w-[var(--content-max)] flex-col gap-4 px-5 py-5">
            {children}
          </div>
        )}
      </div>
    </div>
  );
}

function CommandButton() {
  const show = useCommands((store) => store.show);

  return (
    <Tooltip label="Open the command menu" keys={[MOD, "K"]}>
      <Button tone="ghost" size="small" aria-label="Open the command menu" onClick={show} className="px-2">
        <Search strokeWidth={1.75} />
        <Keys keys={[MOD, "K"]} />
      </Button>
    </Tooltip>
  );
}

export function Tiles({ children }: { children: ReactNode }) {
  return <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-6">{children}</div>;
}

const INKS = {
  good: "text-good-text",
  warning: "text-warning-text",
  critical: "text-critical-text",
} as const;

export function Tile({
  label,
  value,
  note,
  tone,
}: {
  label: string;
  value: ReactNode;
  note?: ReactNode;
  tone?: keyof typeof INKS;
}) {
  return (
    <div
      data-slot="tile"
      className="flex min-w-0 flex-col gap-1 rounded-lg hairline border-line bg-panel px-4 pt-3 pb-3.5"
    >
      <span className="flex items-center gap-1.5 truncate text-mini font-medium text-ink-secondary">
        {tone ? <span className={cn("size-1.5 shrink-0 rounded-full bg-current", INKS[tone])} /> : null}
        {label}
      </span>
      <span
        className={cn(
          "numeric display truncate text-headline font-semibold tracking-tight",
          tone ? INKS[tone] : "text-ink-title",
        )}
      >
        {value}
      </span>
      {note ? <span className="truncate text-mini text-ink-secondary">{note}</span> : null}
    </div>
  );
}

export function Fact({ name, value }: { name: string; value: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-0.5">
      <dt className="text-mini text-ink-secondary">{name}</dt>
      <dd className="numeric m-0 truncate text-small text-ink">{value}</dd>
    </div>
  );
}

export function Facts({ children }: { children: ReactNode }) {
  return (
    <dl className="m-0 grid grid-cols-2 gap-x-6 gap-y-4 px-4 pt-1 pb-4 sm:grid-cols-3">{children}</dl>
  );
}

export function Property({ name, value }: { name: string; value: ReactNode }) {
  return (
    <div className="grid min-h-8 grid-cols-[7.5rem_minmax(0,1fr)] items-center gap-3">
      <dt className="truncate text-small text-ink-secondary">{name}</dt>
      <dd className="numeric m-0 min-w-0 truncate text-small text-ink">{value}</dd>
    </div>
  );
}

export function Properties({ children }: { children: ReactNode }) {
  return <dl className="m-0 flex flex-col px-4 py-2">{children}</dl>;
}
