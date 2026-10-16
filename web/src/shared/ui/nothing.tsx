import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

export function Nothing({
  says,
  offers,
  icon: Icon,
}: {
  says: ReactNode;
  offers?: ReactNode;
  icon?: LucideIcon;
}) {
  return (
    <div
      data-slot="nothing"
      className="flex flex-col items-center justify-center gap-3 px-6 py-14 text-center"
    >
      {Icon ? <Icon className="size-5 text-ink-muted" strokeWidth={1.5} /> : null}
      <p className="m-0 max-w-sm text-small text-ink-secondary">{says}</p>
      {offers}
    </div>
  );
}
