import { X } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "@/shared/lib/cn";
import { Button } from "./button";
import { Tooltip } from "./tooltip";

export function Inspector({
  kicker,
  title,
  subtitle,
  actions,
  onClose,
  children,
}: {
  kicker: ReactNode;
  title: ReactNode;
  subtitle?: ReactNode;
  actions?: ReactNode;
  onClose: () => void;
  children?: ReactNode;
}) {
  return (
    <>
      <header className="flex shrink-0 flex-col gap-3 hairline-b border-line-subtle px-5 pt-2.5 pb-4">
        <div className="flex h-7 items-center justify-between gap-2 select-none">
          <span className="flex min-w-0 items-center gap-2 truncate text-mini font-medium text-ink-secondary">
            {kicker}
          </span>
          <Tooltip label="Close" keys={["Esc"]}>
            <Button tone="ghost" size="icon" aria-label="Close" onClick={onClose} className="-mr-1.5">
              <X strokeWidth={1.75} />
            </Button>
          </Tooltip>
        </div>
        <div className="flex min-w-0 flex-col gap-1.5">
          <h2 className="display m-0 text-large font-semibold break-words text-ink-title select-text">
            {title}
          </h2>
          {subtitle ? (
            <div className="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1 text-small text-ink-secondary">
              {subtitle}
            </div>
          ) : null}
        </div>
        {actions ? <div className="flex flex-wrap items-center gap-1.5">{actions}</div> : null}
      </header>
      {children}
    </>
  );
}

export function Detail({
  title,
  aside,
  flush = false,
  children,
}: {
  title?: ReactNode;
  aside?: ReactNode;
  flush?: boolean;
  children: ReactNode;
}) {
  return (
    <section
      data-slot="detail"
      className={cn(
        "flex min-w-0 shrink-0 flex-col gap-2.5 hairline-b border-line-subtle py-4",
        !flush && "px-5",
      )}
    >
      {title ? (
        <header
          className={cn("flex min-h-5 items-center justify-between gap-2 select-none", flush && "px-5")}
        >
          <h3 className="m-0 text-mini font-medium text-ink-secondary">{title}</h3>
          {aside ? <div className="flex items-center gap-1.5 text-mini text-ink-secondary">{aside}</div> : null}
        </header>
      ) : null}
      {children}
    </section>
  );
}
