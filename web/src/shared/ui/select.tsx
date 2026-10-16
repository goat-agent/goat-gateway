import { Select as Base } from "@base-ui/react/select";
import { Check, ChevronDown } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "@/shared/lib/cn";
import { choice, floating } from "./surface";

export type Choice<Value extends string> = { value: Value; label: ReactNode };

const trigger = {
  small: "h-7 px-2.5",
  medium: "h-8 w-full px-3",
} as const;

export function Select<Value extends string>({
  value,
  choices,
  onChange,
  label,
  size = "small",
  className,
}: {
  value: Value;
  choices: readonly Choice<Value>[];
  onChange: (value: Value) => void;
  label: string;
  size?: keyof typeof trigger;
  className?: string;
}) {
  return (
    <Base.Root
      items={choices}
      value={value}
      onValueChange={(next) => {
        if (next !== null) onChange(next);
      }}
    >
      <Base.Trigger
        aria-label={label}
        data-slot="select"
        className={cn(
          "hover-fade inline-flex min-w-0 items-center justify-between gap-2 rounded-lg hairline border-line bg-raised text-small font-medium text-ink shadow-low outline-none select-none hover:bg-raised-hover focus-visible:border-accent data-[popup-open]:bg-raised-hover",
          trigger[size],
          className,
        )}
      >
        <Base.Value className="truncate" />
        <Base.Icon className="flex shrink-0 text-ink-muted">
          <ChevronDown className="size-3.5" strokeWidth={1.75} />
        </Base.Icon>
      </Base.Trigger>
      <Base.Portal>
        <Base.Positioner
          sideOffset={4}
          align="start"
          alignItemWithTrigger={false}
          className="z-[var(--layer-popover)] outline-none"
        >
          <Base.Popup className={floating}>
            <Base.List className="max-h-[min(var(--available-height),22rem)] overflow-y-auto">
              {choices.map((entry) => (
                <Base.Item key={entry.value} value={entry.value} className={choice}>
                  <Base.ItemText className="min-w-0 grow truncate">{entry.label}</Base.ItemText>
                  <Base.ItemIndicator className="flex">
                    <Check strokeWidth={1.75} />
                  </Base.ItemIndicator>
                </Base.Item>
              ))}
            </Base.List>
          </Base.Popup>
        </Base.Positioner>
      </Base.Portal>
    </Base.Root>
  );
}
