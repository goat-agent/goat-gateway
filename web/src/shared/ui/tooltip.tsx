import { Tooltip as Base } from "@base-ui/react/tooltip";
import type { ReactElement, ReactNode } from "react";
import { Keys } from "./kbd";

export function TooltipProvider({ children }: { children: ReactNode }) {
  return (
    <Base.Provider delay={450} closeDelay={0}>
      {children}
    </Base.Provider>
  );
}

export function Tooltip({
  label,
  keys,
  then,
  side = "bottom",
  children,
}: {
  label: ReactNode;
  keys?: readonly string[];
  then?: boolean;
  side?: "top" | "bottom" | "left" | "right";
  children: ReactElement;
}) {
  return (
    <Base.Root>
      <Base.Trigger render={children} />
      <Base.Portal>
        <Base.Positioner side={side} sideOffset={6} className="z-[var(--layer-tooltip)]">
          <Base.Popup className="popup-motion flex h-7 items-center gap-2 rounded-md hairline border-line-menu bg-menu px-2 text-mini text-ink shadow-medium select-none">
            {label}
            {keys ? <Keys keys={keys} then={then} /> : null}
          </Base.Popup>
        </Base.Positioner>
      </Base.Portal>
    </Base.Root>
  );
}
