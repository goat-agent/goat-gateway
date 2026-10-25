import type { ReactNode } from "react";

export function Split({
  list,
  inspector,
  label,
}: {
  list: ReactNode;
  inspector?: ReactNode;
  label: string;
}) {
  return (
    <div className="relative flex min-h-0 min-w-0 flex-1">
      <div data-slot="list" className="@container min-w-0 flex-1 overflow-y-auto">
        {list}
      </div>
      {inspector ? (
        <aside
          aria-label={label}
          data-slot="inspector"
          className="inspector-motion absolute inset-y-0 right-0 z-10 flex w-[min(var(--inspector-width),100%)] min-h-0 shrink-0 flex-col overflow-y-auto hairline-l border-line bg-panel shadow-high lg:static lg:shadow-none 2xl:w-[calc(var(--inspector-width)+60px)]"
        >
          {inspector}
        </aside>
      ) : null}
    </div>
  );
}
