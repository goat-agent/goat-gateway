import { Toast as Base } from "@base-ui/react/toast";
import { CircleAlert, CircleCheck, X } from "lucide-react";
import type { ReactNode } from "react";

const TONES = ["neutral", "good", "critical"] as const;

type ToastTone = (typeof TONES)[number];

const manager = Base.createToastManager();

export function toast(
  title: ReactNode,
  { says, tone = "neutral", id }: { says?: ReactNode; tone?: ToastTone; id?: string } = {},
) {
  manager.add({
    id,
    title,
    description: says,
    type: tone,
    priority: tone === "critical" ? "high" : "low",
  });
}

export function ToastHost({ children }: { children: ReactNode }) {
  return (
    <Base.Provider toastManager={manager} timeout={8500} limit={3}>
      {children}
      <Base.Portal>
        <Base.Viewport className="fixed right-4 bottom-4 z-[var(--layer-toast)] w-[min(364px,calc(100vw-2rem))] outline-none">
          <Toasts />
        </Base.Viewport>
      </Base.Portal>
    </Base.Provider>
  );
}

function Toasts() {
  const { toasts } = Base.useToastManager();

  return toasts.map((shown) => (
    <Base.Root
      key={shown.id}
      toast={shown}
      data-slot="toast"
      className="rounded-lg hairline border-line-menu bg-menu text-ink shadow-high select-none"
    >
      <Base.Content
        key={shown.updateKey ?? 0}
        data-slot="toast-content"
        data-bumped={shown.updateKey ? "" : undefined}
        className="flex items-start gap-2.5 overflow-hidden py-3 pr-10 pl-3"
      >
        <Glyph tone={toneOf(shown.type)} />
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <Base.Title className="m-0 text-small font-medium text-ink-title" />
          <Base.Description className="m-0 text-mini text-ink-secondary" />
        </div>
        <Base.Close
          aria-label="Dismiss"
          className="hover-fade absolute top-2.5 right-2.5 grid size-6 place-items-center rounded-md text-ink-muted hover:bg-menu-hover hover:text-ink"
        >
          <X className="size-3.5" strokeWidth={1.75} />
        </Base.Close>
      </Base.Content>
    </Base.Root>
  ));
}

function Glyph({ tone }: { tone: ToastTone }) {
  if (tone === "good") {
    return <CircleCheck className="mt-0.5 size-4 shrink-0 text-good" strokeWidth={1.75} />;
  }
  if (tone === "critical") {
    return <CircleAlert className="mt-0.5 size-4 shrink-0 text-critical" strokeWidth={1.75} />;
  }
  return null;
}

function toneOf(type: string | undefined): ToastTone {
  return TONES.find((tone) => tone === type) ?? "neutral";
}
