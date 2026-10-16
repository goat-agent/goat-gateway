import type { ComponentProps } from "react";
import { cn } from "@/shared/lib/cn";

export function Table({
  className,
  contained = true,
  ...props
}: ComponentProps<"table"> & { contained?: boolean }) {
  const table = (
    <table
      data-slot="table"
      className={cn(
        "w-full border-separate border-spacing-0 text-small [&_td:first-child]:pl-4 [&_td:last-child]:pr-4 [&_th:first-child]:pl-4 [&_th:last-child]:pr-4",
        className,
      )}
      {...props}
    />
  );
  return contained ? <div className="w-full overflow-x-auto">{table}</div> : table;
}

export function Head({ className, ...props }: ComponentProps<"thead">) {
  return (
    <thead
      data-slot="table-head"
      className={cn(
        "[&_th]:h-8 [&_th]:hairline-t [&_th]:hairline-b [&_th]:border-line-subtle [&_th]:px-3 [&_th]:text-left [&_th]:text-mini [&_th]:font-medium [&_th]:whitespace-nowrap [&_th]:text-ink-secondary [&_th]:select-none",
        className,
      )}
      {...props}
    />
  );
}

export function Body({ className, ...props }: ComponentProps<"tbody">) {
  return (
    <tbody
      data-slot="table-body"
      className={cn(
        "[&_td]:h-[var(--row-height)] [&_td]:hairline-b [&_td]:border-line-subtle [&_td]:px-3 [&_td]:align-middle [&_td]:whitespace-nowrap [&_tr:last-child_td]:border-b-0",
        className,
      )}
      {...props}
    />
  );
}

export function Foot({ className, ...props }: ComponentProps<"tfoot">) {
  return (
    <tfoot
      data-slot="table-foot"
      className={cn(
        "[&_td]:h-[var(--row-height)] [&_td]:hairline-t [&_td]:border-line [&_td]:px-3 [&_td]:font-medium [&_td]:whitespace-nowrap [&_td]:text-ink-title",
        className,
      )}
      {...props}
    />
  );
}

export function Row({ className, ...props }: ComponentProps<"tr">) {
  return <tr data-slot="table-row" className={cn("hover-fade", className)} {...props} />;
}

export const rowHover = "hover:bg-panel-hover data-[popup-open]:bg-panel-hover";

export function Cell({ className, ...props }: ComponentProps<"td">) {
  return <td className={cn("numeric text-right", className)} {...props} />;
}

export function Count({ className, ...props }: ComponentProps<"th">) {
  return <th className={cn("text-right!", className)} {...props} />;
}
