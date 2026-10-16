import { Check, Copy } from "lucide-react";
import { useCallback, useState } from "react";
import { Button, Dialog, Field, Input, Select } from "@/shared/ui";
import { send, useHappenings, useResource } from "@/shared/api";
import type { Key, User } from "@/entities/user";
import { ConnectGuide } from "./connect-guide";

export function IssueKey({
  open,
  people,
  onClose,
  onIssued,
}: {
  open: boolean;
  people: User[];
  onClose: () => void;
  onIssued: () => void;
}) {
  const [minted, setMinted] = useState<{ id: string; key: string }>();
  const [label, setLabel] = useState("");
  const [owner, setOwner] = useState("");
  const [refused, setRefused] = useState<string>();
  const [copied, setCopied] = useState(false);
  const keys = useResource<{ keys: Key[] }>(minted ? "/api/keys" : undefined);

  const reload = keys.reload;
  useHappenings(
    useCallback(
      (happening) => {
        if (minted && happening === "request_settled") reload();
      },
      [minted, reload],
    ),
  );

  const chosen = owner || people[0]?.id || "";
  const firstSeen = minted
    ? (keys.data?.keys.find((key) => key.id === minted.id)?.last_used_at ?? null)
    : undefined;

  const close = () => {
    setMinted(undefined);
    setLabel("");
    setRefused(undefined);
    setCopied(false);
    onClose();
  };

  return (
    <Dialog
      open={open}
      onClose={close}
      width={minted ? "wide" : "medium"}
      title={minted ? "Connect a client" : "Issue a key"}
      description={
        minted
          ? "This is the only time the key is shown. Only its hash is kept."
          : "One key per program on one machine, so you can tell them apart and revoke one without the others."
      }
    >
      {minted ? (
        <div className="flex flex-col gap-4">
          <div className="flex items-center gap-2">
            <code className="mono block min-w-0 grow rounded-lg hairline border-line bg-base px-3 py-2 break-all text-ink-title select-all">
              {minted.key}
            </code>
            <Button
              size="medium"
              onClick={() => {
                void navigator.clipboard.writeText(minted.key).then(
                  () => setCopied(true),
                  () => setCopied(false),
                );
              }}
            >
              {copied ? <Check strokeWidth={1.75} /> : <Copy strokeWidth={1.75} />}
              {copied ? "Copied" : "Copy"}
            </Button>
          </div>
          <ConnectGuide secret={minted.key} firstSeen={firstSeen} />
          <div className="flex justify-end">
            <Button tone="primary" size="medium" onClick={close}>
              Done
            </Button>
          </div>
        </div>
      ) : (
        <form
          className="flex flex-col gap-4"
          onSubmit={(event) => {
            event.preventDefault();
            setRefused(undefined);
            send<{ id: string; key: string }>("/api/keys", {
              user_id: people.length > 0 ? chosen : undefined,
              label: label.trim(),
            })
              .then((issued) => {
                setMinted(issued);
                onIssued();
              })
              .catch((error: Error) => setRefused(error.message));
          }}
        >
          <Field label="Label" hint="The program and the machine, like “Claude Code on the laptop”." refused={refused}>
            <Input
              autoFocus
              value={label}
              placeholder="Claude Code on the laptop"
              onChange={(event) => setLabel(event.target.value)}
            />
          </Field>
          {people.length > 0 ? (
            <Field label="For">
              <Select
                label="For"
                size="medium"
                value={chosen}
                choices={people.map((person) => ({ value: person.id, label: person.name }))}
                onChange={setOwner}
              />
            </Field>
          ) : null}
          <div className="flex justify-end gap-2 pt-1">
            <Button size="medium" onClick={close}>
              Cancel
            </Button>
            <Button tone="primary" size="medium" type="submit" disabled={label.trim() === ""}>
              Issue
            </Button>
          </div>
        </form>
      )}
    </Dialog>
  );
}
