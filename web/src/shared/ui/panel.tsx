import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/shared/lib/cn";

export function Panel({ className, ...props }: ComponentProps<"section">) {
  return (
    <section
      data-slot="panel"
      className={cn("min-w-0 overflow-hidden rounded-lg hairline border-line bg-panel", className)}
      {...props}
    />
  );
}

export function PanelHead({
  title,
  note,
  children,
}: {
  title: ReactNode;
  note?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <header
      data-slot="panel-head"
      className="flex min-h-11 flex-wrap items-center justify-between gap-x-3 gap-y-1 px-4 py-1.5 select-none"
    >
      <div className="flex min-w-0 items-baseline gap-2">
        <h2 className="m-0 shrink-0 text-small font-medium text-ink-title">{title}</h2>
        {note ? <span className="truncate text-mini text-ink-secondary">{note}</span> : null}
      </div>
      {children ? <div className="flex items-center gap-1.5">{children}</div> : null}
    </header>
  );
}

export function Group({
  title,
  count,
  children,
}: {
  title: ReactNode;
  count?: number;
  children?: ReactNode;
}) {
  return (
    <div
      data-slot="group"
      className="flex h-[var(--group-height)] items-center gap-2 hairline-b border-line-subtle bg-shade px-5 select-none"
    >
      <span className="text-small font-medium text-ink-title">{title}</span>
      {count !== undefined ? (
        <span className="numeric text-small text-ink-secondary">{count}</span>
      ) : null}
      {children ? <div className="ml-auto flex items-center gap-1.5">{children}</div> : null}
    </div>
  );
}
