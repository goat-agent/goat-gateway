import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "react";
import { cn } from "@/shared/lib/cn";

const badge = cva(
  "inline-flex h-5 shrink-0 items-center gap-1.5 rounded-full px-2 text-mini font-medium whitespace-nowrap select-none [&_svg]:size-3",
  {
    variants: {
      tone: {
        neutral: "hairline border-line text-ink-secondary",
        good: "bg-good-tint text-good-text",
        warning: "bg-warning-tint text-warning-text",
        serious: "bg-serious-tint text-serious-text",
        critical: "bg-critical-tint text-critical-text",
        info: "bg-info-tint text-info-text",
      },
    },
    defaultVariants: { tone: "neutral" },
  },
);

export type Tone = NonNullable<VariantProps<typeof badge>["tone"]>;
export type BadgeProps = ComponentProps<"span"> & VariantProps<typeof badge> & { dot?: boolean };

export function Badge({ className, tone, dot = false, children, ...props }: BadgeProps) {
  return (
    <span data-slot="badge" className={cn(badge({ tone }), className)} {...props}>
      {dot ? <span className="size-1.5 shrink-0 rounded-full bg-current" /> : null}
      {children}
    </span>
  );
}

export function Pill({
  className,
  swatch,
  children,
  ...props
}: ComponentProps<"span"> & { swatch?: string }) {
  return (
    <span
      data-slot="pill"
      className={cn(
        "inline-flex h-6 shrink-0 items-center gap-1.5 rounded-full hairline border-line px-2.5 text-mini font-medium whitespace-nowrap text-ink select-none",
        className,
      )}
      {...props}
    >
      {swatch ? (
        <span className="size-2 shrink-0 rounded-full" style={{ background: swatch }} />
      ) : null}
      {children}
    </span>
  );
}
