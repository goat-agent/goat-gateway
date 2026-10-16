import { X } from "lucide-react";

export function FilterChips({
  held,
  onDrop,
}: {
  held: Record<string, string>;
  onDrop: (field: string) => void;
}) {
  const chips = Object.entries(held);
  if (chips.length === 0) return null;

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {chips.map(([field, value]) => (
        <span
          key={field}
          className="inline-flex h-7 items-stretch overflow-hidden rounded-lg hairline border-line bg-raised text-mini font-medium shadow-low select-none"
        >
          <span className="flex items-center px-2 text-ink-secondary capitalize">{field}</span>
          <span className="flex items-center hairline-l border-line px-2 text-ink-muted">is</span>
          <span className="flex max-w-56 items-center hairline-l border-line px-2 text-ink">
            <span className="truncate">{value || "unattributed"}</span>
          </span>
          <button
            type="button"
            aria-label={`Stop filtering by ${field}`}
            className="hover-fade grid w-7 place-items-center hairline-l border-line text-ink-secondary hover:bg-raised-hover hover:text-ink"
            onClick={() => onDrop(field)}
          >
            <X className="size-3" strokeWidth={2} />
          </button>
        </span>
      ))}
    </div>
  );
}
