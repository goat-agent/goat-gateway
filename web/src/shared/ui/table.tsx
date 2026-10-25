import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/shared/lib/cn";

export function Table({
  className,
  sticky = false,
  ...props
}: ComponentProps<"table"> & { sticky?: boolean }) {
  return (
    <table
      data-slot="table"
      className={cn(
        "w-full border-separate border-spacing-0 text-small [&_td:first-child]:pl-5 [&_td:last-child]:pr-5 [&_th:first-child]:pl-5 [&_th:last-child]:pr-5",
        sticky &&
          "[&_thead_th]:sticky [&_thead_th]:top-0 [&_thead_th]:z-[1] [&_thead_th]:bg-panel",
        className,
      )}
      {...props}
    />
  );
}

export function Head({ className, ...props }: ComponentProps<"thead">) {
  return (
    <thead
      data-slot="table-head"
      className={cn(
        "[&_th]:h-8 [&_th]:hairline-b [&_th]:border-line-subtle [&_th]:px-2.5 [&_th]:text-left [&_th]:text-mini [&_th]:font-medium [&_th]:whitespace-nowrap [&_th]:text-ink-secondary [&_th]:select-none",
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
        "[&_td]:h-[var(--row-height)] [&_td]:hairline-b [&_td]:border-line-subtle [&_td]:px-2.5 [&_td]:align-middle [&_td]:whitespace-nowrap",
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
        "[&_td]:h-[var(--row-height)] [&_td]:px-2.5 [&_td]:font-medium [&_td]:whitespace-nowrap [&_td]:text-ink-title",
        className,
      )}
      {...props}
    />
  );
}

export function Row({
  className,
  selected = false,
  ...props
}: ComponentProps<"tr"> & { selected?: boolean }) {
  return (
    <tr
      data-slot="table-row"
      data-selected={selected ? "" : undefined}
      className={cn("hover-fade", className)}
      {...props}
    />
  );
}

export const rowHover =
  "hover:bg-panel-hover data-[popup-open]:bg-panel-hover data-[selected]:bg-selected";

export function GroupRow({
  span,
  title,
  count,
}: {
  span: number;
  title: ReactNode;
  count?: number;
}) {
  return (
    <tr data-slot="group-row">
      <td colSpan={span} className="h-9! bg-shade select-none">
        <span className="flex items-center gap-2 text-small">
          <span className="font-medium text-ink-title">{title}</span>
          {count !== undefined ? <span className="numeric text-ink-muted">{count}</span> : null}
        </span>
      </td>
    </tr>
  );
}

export function Cell({ className, ...props }: ComponentProps<"td">) {
  return <td className={cn("numeric text-right", className)} {...props} />;
}

export function Count({ className, ...props }: ComponentProps<"th">) {
  return <th className={cn("text-right!", className)} {...props} />;
}
