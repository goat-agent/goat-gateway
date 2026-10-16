import { ContextMenu as BaseContext } from "@base-ui/react/context-menu";
import { Menu as Base } from "@base-ui/react/menu";
import type { LucideIcon } from "lucide-react";
import type { ReactElement, ReactNode } from "react";
import { cn } from "@/shared/lib/cn";
import { Keys } from "./kbd";
import { choice, floating } from "./surface";

export function Menu({
  trigger,
  align = "end",
  children,
}: {
  trigger: ReactElement;
  align?: "start" | "center" | "end";
  children: ReactNode;
}) {
  return (
    <Base.Root>
      <Base.Trigger render={trigger} />
      <Base.Portal>
        <Base.Positioner
          sideOffset={4}
          align={align}
          className="z-[var(--layer-popover)] outline-none"
        >
          <Base.Popup className={floating}>{children}</Base.Popup>
        </Base.Positioner>
      </Base.Portal>
    </Base.Root>
  );
}

export function ContextMenu({
  menu,
  render,
  children,
}: {
  menu: ReactNode;
  render?: ReactElement;
  children: ReactNode;
}) {
  return (
    <BaseContext.Root>
      <BaseContext.Trigger render={render}>{children}</BaseContext.Trigger>
      <BaseContext.Portal>
        <BaseContext.Positioner className="z-[var(--layer-popover)] outline-none">
          <BaseContext.Popup className={floating}>{menu}</BaseContext.Popup>
        </BaseContext.Positioner>
      </BaseContext.Portal>
    </BaseContext.Root>
  );
}

export function MenuItem({
  icon: Icon,
  keys,
  danger = false,
  disabled,
  onClick,
  children,
}: {
  icon?: LucideIcon;
  keys?: readonly string[];
  danger?: boolean;
  disabled?: boolean;
  onClick?: () => void;
  children: ReactNode;
}) {
  return (
    <Base.Item
      disabled={disabled}
      onClick={() => onClick?.()}
      className={cn(choice, danger && "text-critical-text [&_svg]:text-critical-text")}
    >
      {Icon ? <Icon strokeWidth={1.75} /> : null}
      <span className="min-w-0 grow truncate">{children}</span>
      {keys ? <Keys keys={keys} /> : null}
    </Base.Item>
  );
}

export function MenuLabel({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-7 items-center px-2 text-mini font-medium text-ink-secondary">
      {children}
    </div>
  );
}

export function MenuSeparator() {
  return <Base.Separator className="-mx-1 my-1 h-[var(--hairline)] bg-line-menu" />;
}
