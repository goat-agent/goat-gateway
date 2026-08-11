import type { ComponentProps } from "react";
import { cn } from "@/shared/lib/cn";

export function Table({ className, ...props }: ComponentProps<"table">) {
  return (
    <div className="w-full overflow-x-auto">
      <table
        data-slot="table"
        className={cn("w-full border-collapse text-small", className)}
        {...props}
      />
    </div>
  );
}

export function Head({ className, ...props }: ComponentProps<"thead">) {
  return (
    <thead
      data-slot="table-head"
      className={cn(
        "[&_th]:h-8 [&_th]:border-b [&_th]:border-line-subtle [&_th]:px-3 [&_th]:text-left [&_th]:text-micro [&_th]:font-medium [&_th]:tracking-wide [&_th]:text-ink-muted [&_th]:uppercase [&_th]:whitespace-nowrap",
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
        "[&_td]:h-9 [&_td]:border-b [&_td]:border-line-subtle [&_td]:px-3 [&_td]:align-middle [&_td]:whitespace-nowrap [&_tr:last-child_td]:border-b-0",
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
        "[&_td]:h-9 [&_td]:border-t [&_td]:border-line [&_td]:px-3 [&_td]:font-medium [&_td]:whitespace-nowrap",
        className,
      )}
      {...props}
    />
  );
}

export function Row(props: ComponentProps<"tr">) {
  return <tr data-slot="table-row" {...props} />;
}

export function Cell({ className, ...props }: ComponentProps<"td">) {
  return <td className={cn("numeric text-right", className)} {...props} />;
}

export function Count({ className, ...props }: ComponentProps<"th">) {
  return <th className={cn("!text-right", className)} {...props} />;
}
