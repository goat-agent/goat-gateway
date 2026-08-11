import { useEffect, useRef, type ReactNode } from "react";

export function Dialog({
  open,
  onClose,
  title,
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
}) {
  const held = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const element = held.current;
    if (!element) return;
    if (open && !element.open) element.showModal();
    if (!open && element.open) element.close();
  }, [open]);

  return (
    <dialog
      ref={held}
      data-slot="dialog"
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
      onClick={(event) => {
        if (event.target === held.current) onClose();
      }}
      className="m-auto w-[min(30rem,calc(100vw-2rem))] rounded-md border border-line bg-overlay p-0 text-ink backdrop:bg-black/60"
    >
      <header className="flex h-11 items-center border-b border-line-subtle px-4 text-small font-semibold">
        {title}
      </header>
      <div className="p-4">{children}</div>
    </dialog>
  );
}
