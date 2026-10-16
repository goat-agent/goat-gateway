import { AlertDialog } from "@base-ui/react/alert-dialog";
import type { ReactNode } from "react";
import { create } from "zustand";
import { Button } from "./button";

type Question = {
  title: string;
  says?: ReactNode;
  action: string;
  danger: boolean;
  answer: (yes: boolean) => void;
};

const useQuestion = create<{ asked: Question | undefined; open: boolean }>(() => ({
  asked: undefined,
  open: false,
}));

export function confirmed({
  title,
  says,
  action = "Confirm",
  danger = true,
}: {
  title: string;
  says?: ReactNode;
  action?: string;
  danger?: boolean;
}): Promise<boolean> {
  return new Promise((answer) => {
    useQuestion.getState().asked?.answer(false);
    useQuestion.setState({ open: true, asked: { title, says, action, danger, answer } });
  });
}

export function ConfirmationHost() {
  const asked = useQuestion((store) => store.asked);
  const open = useQuestion((store) => store.open);
  if (!asked) return null;

  return (
    <ConfirmDialog
      open={open}
      title={asked.title}
      says={asked.says}
      action={asked.action}
      danger={asked.danger}
      onAnswer={(yes) => {
        asked.answer(yes);
        useQuestion.setState({ open: false });
      }}
    />
  );
}

export function ConfirmDialog({
  open,
  title,
  says,
  action,
  danger = true,
  onAnswer,
}: {
  open: boolean;
  title: string;
  says?: ReactNode;
  action: string;
  danger?: boolean;
  onAnswer: (yes: boolean) => void;
}) {
  return (
    <AlertDialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!next) onAnswer(false);
      }}
    >
      <AlertDialog.Portal>
        <AlertDialog.Backdrop className="scrim-motion fixed inset-0 z-[var(--layer-dialog)] bg-scrim" />
        <AlertDialog.Viewport className="fixed inset-0 z-[var(--layer-dialog)] flex items-start justify-center px-4 pt-[13vh]">
          <AlertDialog.Popup className="dialog-motion flex w-[min(26rem,100%)] origin-top flex-col gap-4 rounded-xl hairline border-line-menu bg-overlay p-5 text-ink shadow-high outline-none">
            <div className="flex flex-col gap-1.5">
              <AlertDialog.Title className="m-0 text-regular font-semibold text-ink-title">
                {title}
              </AlertDialog.Title>
              {says ? (
                <AlertDialog.Description className="m-0 text-small text-ink-secondary">
                  {says}
                </AlertDialog.Description>
              ) : null}
            </div>
            <div className="flex justify-end gap-2">
              <AlertDialog.Close render={<Button size="medium" />}>Cancel</AlertDialog.Close>
              <Button tone={danger ? "danger" : "primary"} size="medium" onClick={() => onAnswer(true)}>
                {action}
              </Button>
            </div>
          </AlertDialog.Popup>
        </AlertDialog.Viewport>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
