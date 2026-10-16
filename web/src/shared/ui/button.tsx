import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "react";
import { Link, type LinkProps } from "react-router-dom";
import { cn } from "@/shared/lib/cn";

const button = cva(
  "hover-fade inline-flex shrink-0 items-center justify-center gap-1.5 rounded-lg font-sans font-medium whitespace-nowrap select-none disabled:pointer-events-none disabled:opacity-45 [&_svg]:size-4 [&_svg]:shrink-0",
  {
    variants: {
      tone: {
        primary:
          "bg-accent text-accent-text shadow-low hover:bg-accent-hover data-[popup-open]:bg-accent-hover",
        secondary:
          "hairline border-line bg-raised text-ink shadow-low hover:bg-raised-hover data-[popup-open]:bg-raised-hover",
        ghost:
          "bg-transparent text-ink-secondary hover:bg-raised-hover hover:text-ink data-[popup-open]:bg-raised-hover data-[popup-open]:text-ink",
        danger: "bg-critical text-white shadow-low hover:brightness-110",
      },
      size: {
        small: "h-7 px-2.5 text-small",
        medium: "h-8 px-3 text-small",
        icon: "size-7 [&_svg]:size-4",
      },
    },
    defaultVariants: { tone: "secondary", size: "small" },
  },
);

export type ButtonProps = ComponentProps<"button"> & VariantProps<typeof button>;

export function ButtonLink({
  className,
  tone,
  size,
  ...props
}: LinkProps & VariantProps<typeof button>) {
  return <Link data-slot="button" className={cn(button({ tone, size }), className)} {...props} />;
}

export function Button({ className, tone, size, type = "button", ...props }: ButtonProps) {
  return (
    <button
      data-slot="button"
      type={type}
      className={cn(button({ tone, size }), className)}
      {...props}
    />
  );
}
