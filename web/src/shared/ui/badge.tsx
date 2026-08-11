import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "react";
import { cn } from "@/shared/lib/cn";

const badge = cva(
  "inline-flex h-5 items-center gap-1 rounded-sm px-1.5 text-micro font-medium whitespace-nowrap",
  {
    variants: {
      tone: {
        neutral: "bg-raised text-ink-secondary",
        good: "bg-[color-mix(in_srgb,var(--good)_18%,transparent)] text-[var(--good)]",
        warning: "bg-[color-mix(in_srgb,var(--warning)_18%,transparent)] text-[var(--warning)]",
        serious: "bg-[color-mix(in_srgb,var(--serious)_18%,transparent)] text-[var(--serious)]",
        critical: "bg-[color-mix(in_srgb,var(--critical)_18%,transparent)] text-[var(--critical)]",
      },
    },
    defaultVariants: { tone: "neutral" },
  },
);

export type Tone = NonNullable<VariantProps<typeof badge>["tone"]>;
export type BadgeProps = ComponentProps<"span"> & VariantProps<typeof badge>;

export function Badge({ className, tone, ...props }: BadgeProps) {
  return <span data-slot="badge" className={cn(badge({ tone }), className)} {...props} />;
}
