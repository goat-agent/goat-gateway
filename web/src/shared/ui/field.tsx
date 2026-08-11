import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/shared/lib/cn";

const shell =
  "h-8 w-full rounded-sm border border-line bg-base px-2 text-small text-ink placeholder:text-ink-muted focus:border-line-strong focus:outline-none disabled:opacity-40";

export function Input({ className, ...props }: ComponentProps<"input">) {
  return <input data-slot="input" className={cn(shell, className)} {...props} />;
}

export function Select({ className, ...props }: ComponentProps<"select">) {
  return <select data-slot="select" className={cn(shell, "cursor-pointer pr-6", className)} {...props} />;
}

export function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: ReactNode;
  children: ReactNode;
}) {
  return (
    <label data-slot="field" className="flex flex-col gap-1.5">
      <span className="text-micro font-medium tracking-wide text-ink-secondary uppercase">
        {label}
      </span>
      {children}
      {hint ? <span className="text-micro text-ink-muted">{hint}</span> : null}
    </label>
  );
}
