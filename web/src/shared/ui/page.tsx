import { CircleAlert } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "@/shared/lib/cn";

export function Page({
  title,
  count,
  actions,
  toolbar,
  refused,
  scroll = true,
  children,
}: {
  title: ReactNode;
  count?: number;
  actions?: ReactNode;
  toolbar?: ReactNode;
  refused?: string;
  scroll?: boolean;
  children: ReactNode;
}) {
  return (
    <div data-slot="page" className="flex h-full min-h-0 flex-col">
      <header className="flex h-[var(--bar-height)] shrink-0 items-center gap-2 hairline-b border-line-subtle pr-3 pl-5 select-none">
        <h1 className="m-0 min-w-0 truncate text-small font-medium text-ink-title">{title}</h1>
        {count !== undefined ? (
          <span className="numeric text-small text-ink-muted">{count.toLocaleString("en-US")}</span>
        ) : null}
        {actions ? <div className="ml-auto flex shrink-0 items-center gap-1.5">{actions}</div> : null}
      </header>

      {toolbar ? (
        <div className="flex min-h-10 shrink-0 flex-wrap items-center gap-2 hairline-b border-line-subtle px-5 py-1.5 select-none">
          {toolbar}
        </div>
      ) : null}

      {refused ? (
        <div className="flex shrink-0 items-center gap-2 hairline-b border-line-subtle bg-critical-tint px-5 py-2 text-small text-critical-text">
          <CircleAlert className="size-4 shrink-0" strokeWidth={1.75} />
          {refused}
        </div>
      ) : null}

      {scroll ? (
        <div className="min-h-0 flex-1 overflow-y-auto">{children}</div>
      ) : (
        <div className="relative flex min-h-0 flex-1">{children}</div>
      )}
    </div>
  );
}

export function Section({
  title,
  note,
  aside,
  className,
  children,
}: {
  title: ReactNode;
  note?: ReactNode;
  aside?: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  return (
    <section data-slot="section" className={cn("flex min-w-0 flex-col", className)}>
      <header className="flex min-h-11 flex-wrap items-center gap-x-3 gap-y-1 px-5 py-1.5 select-none">
        <h2 className="m-0 shrink-0 text-small font-medium text-ink-title">{title}</h2>
        {note ? <span className="min-w-0 truncate text-mini text-ink-secondary">{note}</span> : null}
        {aside ? <div className="ml-auto flex items-center gap-1.5">{aside}</div> : null}
      </header>
      {children}
    </section>
  );
}

const INKS = {
  good: "text-good-text",
  warning: "text-warning-text",
  critical: "text-critical-text",
} as const;

export function Stats({ children }: { children: ReactNode }) {
  return (
    <div data-slot="stats" className="flex shrink-0 overflow-x-auto hairline-b border-line-subtle">
      {children}
    </div>
  );
}

export function Stat({
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
      data-slot="stat"
      className="flex min-w-40 flex-1 flex-col gap-1 border-line-subtle px-5 pt-4 pb-4.5 not-first:hairline-l"
    >
      <span className="truncate text-mini text-ink-secondary">{label}</span>
      <span
        className={cn(
          "numeric display truncate text-headline font-semibold",
          tone ? INKS[tone] : "text-ink-title",
        )}
      >
        {value}
      </span>
      {note ? <span className="truncate text-mini text-ink-muted">{note}</span> : null}
    </div>
  );
}

export function Fact({ name, value }: { name: string; value: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-0.5">
      <dt className="text-mini text-ink-secondary">{name}</dt>
      <dd className="numeric m-0 truncate text-small text-ink-title">{value}</dd>
    </div>
  );
}

export function Facts({ children }: { children: ReactNode }) {
  return <dl className="m-0 grid grid-cols-3 gap-x-4 gap-y-3">{children}</dl>;
}

export function Property({ name, value }: { name: string; value: ReactNode }) {
  return (
    <div className="grid min-h-8 grid-cols-[7.5rem_minmax(0,1fr)] items-center gap-3">
      <dt className="truncate text-small text-ink-secondary">{name}</dt>
      <dd className="numeric m-0 flex min-w-0 items-center text-small text-ink">{value}</dd>
    </div>
  );
}

export function Properties({ children }: { children: ReactNode }) {
  return <dl className="m-0 flex flex-col">{children}</dl>;
}
