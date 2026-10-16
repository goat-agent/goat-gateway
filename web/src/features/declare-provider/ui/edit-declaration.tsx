import { useEffect, useState } from "react";
import { Dialog } from "@/shared/ui";
import { useResource } from "@/shared/api";
import { DeclarationEditor } from "./declaration-editor";

export function EditDeclaration({
  id,
  label,
  onClose,
  onSaved,
}: {
  id: string | undefined;
  label: string;
  onClose: () => void;
  onSaved: (id: string) => void;
}) {
  const held = useResource<{ id: string; declaration: string }>(
    id === undefined ? undefined : `/api/providers/${encodeURIComponent(id)}`,
  );
  const [text, setText] = useState("");
  const loaded = held.data?.declaration;

  useEffect(() => {
    if (loaded !== undefined) setText(loaded);
  }, [loaded]);

  return (
    <Dialog
      open={id !== undefined}
      onClose={onClose}
      width="wide"
      title={`Edit ${label}`}
      description="A built-in provider edited here is overridden in config.toml. Removing the override brings the built-in one back."
    >
      {held.error ? (
        <p className="m-0 text-small text-critical-text">{held.error}</p>
      ) : (
        <DeclarationEditor
          id={id ?? ""}
          onId={() => undefined}
          fixed
          text={text}
          onText={setText}
          onSaved={(saved) => {
            onClose();
            onSaved(saved);
          }}
        />
      )}
    </Dialog>
  );
}
