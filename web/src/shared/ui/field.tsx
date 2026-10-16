import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/shared/lib/cn";

const shell =
  "hover-fade h-8 w-full min-w-0 rounded-lg hairline border-line bg-raised px-3 text-small text-ink shadow-low outline-none placeholder:text-ink-muted hover:border-line-solid focus:border-accent focus-visible:outline-none disabled:opacity-45 aria-invalid:border-critical";

export function Input({
  className,
  lead,
  ...props
}: ComponentProps<"input"> & { lead?: ReactNode }) {
  if (!lead) {
    return <input data-slot="input" className={cn(shell, className)} {...props} />;
  }
  return (
    <span data-slot="input" className={cn("relative flex items-center", className)}>
      <span className="pointer-events-none absolute left-2.5 flex text-ink-muted [&_svg]:size-3.5">
        {lead}
      </span>
      <input className={cn(shell, "h-full pl-8")} {...props} />
    </span>
  );
}

export function Field({
  label,
  hint,
  refused,
  children,
}: {
  label: string;
  hint?: ReactNode;
  refused?: string;
  children: ReactNode;
}) {
  return (
    <div data-slot="field" className="flex flex-col gap-1.5">
      <label className="flex flex-col gap-1.5">
        <span className="text-mini font-medium text-ink">{label}</span>
        {children}
      </label>
      {refused ? (
        <span role="alert" className="text-mini text-critical-text">
          {refused}
        </span>
      ) : hint ? (
        <span className="text-mini text-ink-secondary">{hint}</span>
      ) : null}
    </div>
  );
}
