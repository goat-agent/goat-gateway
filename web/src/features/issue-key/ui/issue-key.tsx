import { useState } from "react";
import { send } from "@/shared/api";
import { Button, Dialog, Field, Input, Select } from "@/shared/ui";
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
  const [owner, setOwner] = useState("");
  const [label, setLabel] = useState("");
  const [minted, setMinted] = useState<string>();

  const close = () => {
    setMinted(undefined);
    setLabel("");
    onClose();
  };

  return (
    <Dialog open={open} onClose={close} title={minted ? "Copy this now" : "Issue a key"}>
      {minted ? (
        <div className="flex flex-col gap-3">
          <p className="m-0 text-small text-ink-secondary">
            This is the only time the key is shown. Only its hash is kept.
          </p>
          <code className="block rounded-sm border border-line bg-base p-2 font-mono text-code break-all">
            {minted}
          </code>
          <Button tone="primary" onClick={close}>
            Done
          </Button>
        </div>
      ) : (
        <form
          className="flex flex-col gap-3"
          onSubmit={(event) => {
            event.preventDefault();
            send<{ key: string }>("/api/keys", {
              user_id: owner || people[0]?.id,
              label: label.trim(),
            }).then((issued) => {
              setMinted(issued.key);
              onIssued();
            });
          }}
        >
          <Field label="For">
            <Select value={owner} onChange={(event) => setOwner(event.target.value)}>
              {people.map((person) => (
                <option key={person.id} value={person.id}>
                  {person.name}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Label" hint="So you can tell your machines apart later.">
            <Input
              autoFocus
              value={label}
              placeholder="laptop"
              onChange={(event) => setLabel(event.target.value)}
            />
          </Field>
          <Button tone="primary" type="submit" disabled={label.trim() === ""}>
            Issue
          </Button>
        </form>
      )}
    </Dialog>
  );
}
