import { Check, Copy } from "lucide-react";
import { useState } from "react";
import { Button, Dialog, Field, Input, Select } from "@/shared/ui";
import { send } from "@/shared/api";
import type { User } from "@/entities/user";

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
  const [minted, setMinted] = useState<string>();
  const [label, setLabel] = useState("");
  const [owner, setOwner] = useState(people[0]?.id ?? "");
  const [refused, setRefused] = useState<string>();
  const [copied, setCopied] = useState(false);

  const chosen = owner || people[0]?.id || "";

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
      title={minted ? "Copy this now" : "Issue a key"}
      description={
        minted
          ? "This is the only time the key is shown. Only its hash is kept."
          : "A client presents this key to the gateway. Requests made with it are attributed to its person."
      }
    >
      {minted ? (
        <div className="flex flex-col gap-4">
          <code className="mono block rounded-lg hairline border-line bg-base px-3 py-2.5 break-all text-ink-title select-all">
            {minted}
          </code>
          <div className="flex justify-end gap-2">
            <Button
              size="medium"
              onClick={() => {
                void navigator.clipboard.writeText(minted).then(
                  () => setCopied(true),
                  () => setCopied(false),
                );
              }}
            >
              {copied ? <Check strokeWidth={1.75} /> : <Copy strokeWidth={1.75} />}
              {copied ? "Copied" : "Copy"}
            </Button>
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
            send<{ key: string }>("/api/keys", {
              user_id: owner || people[0]?.id,
              label: label.trim(),
            })
              .then((issued) => {
                setMinted(issued.key);
                onIssued();
              })
              .catch((error: Error) => setRefused(error.message));
          }}
        >
          <Field label="Label" hint="So you can tell your machines apart later." refused={refused}>
            <Input
              value={label}
              placeholder="laptop"
              onChange={(event) => setLabel(event.target.value)}
            />
          </Field>
          <Field label="For">
            <Select
              label="For"
              size="medium"
              value={chosen}
              choices={people.map((person) => ({ value: person.id, label: person.name }))}
              onChange={setOwner}
            />
          </Field>
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
