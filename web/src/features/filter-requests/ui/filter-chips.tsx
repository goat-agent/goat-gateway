import { X } from "lucide-react";
import { Badge } from "@/shared/ui";

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
        <Badge key={field} className="gap-1.5 pr-1">
          {field}: {value}
          <button
            type="button"
            aria-label={`Stop filtering by ${field}`}
            className="grid size-3.5 place-items-center rounded-[2px] hover:bg-line"
            onClick={() => onDrop(field)}
          >
            <X className="size-2.5" />
          </button>
        </Badge>
      ))}
    </div>
  );
}
