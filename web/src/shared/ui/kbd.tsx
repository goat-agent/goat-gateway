import type { ReactNode } from "react";
import { cn } from "@/shared/lib/cn";

export function Kbd({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <kbd
      data-slot="kbd"
      className={cn(
        "inline-flex h-[18px] min-w-[18px] items-center justify-center rounded-[4px] hairline border-line-solid bg-raised px-1 font-sans text-micro leading-none font-medium text-ink-secondary",
        className,
      )}
    >
      {children}
    </kbd>
  );
}

export function Keys({ keys, then = false }: { keys: readonly string[]; then?: boolean }) {
  return (
    <span data-slot="keys" className="inline-flex items-center gap-1">
      {keys.map((key, index) => (
        <span key={`${key}-${index}`} className="inline-flex items-center gap-1">
          {then && index > 0 ? <span className="text-micro text-ink-muted">then</span> : null}
          <Kbd>{key}</Kbd>
        </span>
      ))}
    </span>
  );
}
