import { Dialog as Base } from "@base-ui/react/dialog";
import { X } from "lucide-react";
import { useRef, type ReactNode } from "react";
import { cn } from "@/shared/lib/cn";
import { Button } from "./button";

const WIDTHS = {
  narrow: "w-[min(26rem,100%)]",
  medium: "w-[min(32rem,100%)]",
  wide: "w-[min(40rem,100%)]",
} as const;

export function Dialog({
  open,
  onClose,
  title,
  description,
  width = "medium",
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  description?: ReactNode;
  width?: keyof typeof WIDTHS;
  children: ReactNode;
}) {
  const held = useRef<HTMLDivElement>(null);

  return (
    <Base.Root
      open={open}
      onOpenChange={(next) => {
        if (!next) onClose();
      }}
    >
      <Base.Portal>
        <Base.Backdrop className="scrim-motion fixed inset-0 z-[var(--layer-dialog)] bg-scrim" />
        <Base.Viewport className="fixed inset-0 z-[var(--layer-dialog)] flex items-start justify-center overflow-y-auto px-4 pt-[13vh] pb-8">
          <Base.Popup
            ref={held}
            data-slot="dialog"
            initialFocus={() => firstField(held.current)}
            className={cn(
              "dialog-motion relative flex origin-top flex-col rounded-xl hairline border-line-menu bg-overlay text-ink shadow-high outline-none",
              WIDTHS[width],
            )}
          >
            <header className="flex flex-col gap-1 px-5 pt-4 pr-12">
              <Base.Title className="m-0 text-regular font-semibold text-ink-title">
                {title}
              </Base.Title>
              {description ? (
                <Base.Description className="m-0 text-small text-ink-secondary">
                  {description}
                </Base.Description>
              ) : null}
            </header>
            <div className="px-5 pt-4 pb-5">{children}</div>
            <Base.Close
              render={<Button tone="ghost" size="icon" aria-label="Close" className="absolute top-3 right-3" />}
            >
              <X strokeWidth={1.75} />
            </Base.Close>
          </Base.Popup>
        </Base.Viewport>
      </Base.Portal>
    </Base.Root>
  );
}

function firstField(popup: HTMLElement | null) {
  const found = popup?.querySelector("input:not([type=hidden]), textarea");
  if (found instanceof HTMLElement) return found;
  return popup ?? true;
}
