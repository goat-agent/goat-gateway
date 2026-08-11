import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "react";
import { cn } from "@/shared/lib/cn";

const button = cva(
  "inline-flex items-center justify-center gap-1.5 rounded-sm border font-sans whitespace-nowrap transition-colors disabled:pointer-events-none disabled:opacity-40 [&_svg]:size-3.5 [&_svg]:shrink-0",
  {
    variants: {
      tone: {
        primary: "border-transparent bg-ink text-base hover:opacity-90",
        plain: "border-line bg-raised text-ink hover:bg-overlay",
        quiet: "border-transparent bg-transparent text-ink-secondary hover:bg-raised hover:text-ink",
        grave:
          "border-transparent bg-transparent text-[var(--critical)] hover:bg-[color-mix(in_srgb,var(--critical)_14%,transparent)]",
      },
      size: {
        base: "h-8 px-3 text-small",
        small: "h-7 px-2 text-micro",
        icon: "size-8",
      },
    },
    defaultVariants: { tone: "plain", size: "base" },
  },
);

export type ButtonProps = ComponentProps<"button"> & VariantProps<typeof button>;

export function Button({ className, tone, size, ...props }: ButtonProps) {
  return <button data-slot="button" className={cn(button({ tone, size }), className)} {...props} />;
}
