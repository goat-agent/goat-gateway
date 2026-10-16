import { useState, type ReactNode } from "react";
import { Button, Field, Input, TextArea } from "@/shared/ui";
import { place } from "@/shared/api";

const ID_SHAPE = /^[a-z0-9][a-z0-9_-]*$/;

export function DeclarationEditor({
  id,
  onId,
  fixed,
  text,
  onText,
  onSaved,
  children,
}: {
  id: string;
  onId: (id: string) => void;
  fixed: boolean;
  text: string;
  onText: (text: string) => void;
  onSaved: (id: string) => void;
  children?: ReactNode;
}) {
  const [refused, setRefused] = useState<string>();
  const [saving, setSaving] = useState(false);
  const shaped = ID_SHAPE.test(id);

  return (
    <form
      className="flex flex-col gap-4"
      onSubmit={(event) => {
        event.preventDefault();
        setRefused(undefined);
        setSaving(true);
        place(`/api/providers/${encodeURIComponent(id)}`, { declaration: text })
          .then(() => onSaved(id))
          .catch((error: Error) => setRefused(error.message))
          .finally(() => setSaving(false));
      }}
    >
      <Field
        label="Id"
        hint={
          fixed
            ? "Saving writes this declaration to config.toml in place of the one it has now."
            : "Lowercase letters, digits, - and _. Models are routed to it by this id."
        }
        refused={id !== "" && !shaped ? "Use lowercase letters, digits, - and _ only." : undefined}
      >
        <Input
          className="mono"
          value={id}
          disabled={fixed}
          onChange={(event) => onId(event.target.value.trim().toLowerCase())}
        />
      </Field>
      <Field
        label="Declaration"
        hint="The same TOML as a [provider.<id>] table in config.toml. The gateway checks it before anything is written."
        refused={refused}
      >
        <TextArea
          rows={14}
          value={text}
          aria-invalid={refused ? true : undefined}
          onChange={(event) => onText(event.target.value)}
        />
      </Field>
      <div className="flex items-center justify-between gap-2 pt-1">
        <span className="flex items-center gap-1">{children}</span>
        <Button type="submit" tone="primary" size="medium" disabled={!shaped || text.trim() === "" || saving}>
          {saving ? "Saving…" : "Save and reload"}
        </Button>
      </div>
    </form>
  );
}
