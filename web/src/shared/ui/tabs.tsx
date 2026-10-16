import { Tabs as BaseTabs } from "@base-ui/react/tabs";
import { Toggle } from "@base-ui/react/toggle";
import { ToggleGroup } from "@base-ui/react/toggle-group";
import { cn } from "@/shared/lib/cn";
import type { Choice } from "./select";

export function SegmentedControl<Value extends string>({
  value,
  choices,
  onChange,
  label,
  className,
}: {
  value: Value;
  choices: readonly Choice<Value>[];
  onChange: (value: Value) => void;
  label: string;
  className?: string;
}) {
  return (
    <ToggleGroup
      aria-label={label}
      value={[value]}
      onValueChange={(next) => {
        const [picked] = next;
        if (picked !== undefined) onChange(picked);
      }}
      className={cn(
        "inline-flex h-7 items-center gap-0.5 rounded-lg hairline border-line bg-base p-0.5 select-none",
        className,
      )}
    >
      {choices.map((entry) => (
        <Toggle
          key={entry.value}
          value={entry.value}
          className="hover-fade h-[22px] rounded-md px-2 text-mini font-medium whitespace-nowrap text-ink-secondary outline-none hover:text-ink focus-visible:outline-1 focus-visible:outline-accent data-[pressed]:bg-raised data-[pressed]:text-ink data-[pressed]:shadow-low"
        >
          {entry.label}
        </Toggle>
      ))}
    </ToggleGroup>
  );
}

export function Tabs<Value extends string>({
  value,
  choices,
  onChange,
  label,
}: {
  value: Value;
  choices: readonly Choice<Value>[];
  onChange: (value: Value) => void;
  label: string;
}) {
  return (
    <BaseTabs.Root
      value={value}
      onValueChange={(next) => {
        const picked = choices.find((entry) => entry.value === next);
        if (picked) onChange(picked.value);
      }}
    >
      <BaseTabs.List aria-label={label} className="relative flex h-11 items-stretch gap-4 select-none">
        {choices.map((entry) => (
          <BaseTabs.Tab
            key={entry.value}
            value={entry.value}
            className="hover-fade flex items-center text-small font-medium text-ink-secondary outline-none hover:text-ink focus-visible:text-ink data-[active]:text-ink-title"
          >
            {entry.label}
          </BaseTabs.Tab>
        ))}
        <BaseTabs.Indicator className="absolute bottom-0 left-[var(--active-tab-left)] h-px w-[var(--active-tab-width)] bg-ink-title transition-[left,width] duration-[var(--speed-regular)] ease-out-quint" />
      </BaseTabs.List>
    </BaseTabs.Root>
  );
}
