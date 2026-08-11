import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/shared/lib/cn";

export function Panel({ className, ...props }: ComponentProps<"section">) {
  return (
    <section
      data-slot="panel"
      className={cn("rounded-md border border-line-subtle bg-panel", className)}
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
      className="flex min-h-11 items-center justify-between gap-3 border-b border-line-subtle px-3"
    >
      <div className="flex items-baseline gap-2">
        <h2 className="text-small font-semibold text-ink">{title}</h2>
        {note ? <span className="text-micro text-ink-muted">{note}</span> : null}
      </div>
      {children ? <div className="flex items-center gap-1.5">{children}</div> : null}
    </header>
  );
}
