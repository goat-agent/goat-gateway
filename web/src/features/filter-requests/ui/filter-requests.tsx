import { useState } from "react";
import type { URLSearchParamsInit } from "react-router-dom";
import { Button, Input, Select } from "@/shared/ui";

const STATUSES = ["", "ok", "error", "in_flight", "abandoned"];

export function FilterRequests({
  params,
  onChange,
}: {
  params: URLSearchParams;
  onChange: (next: URLSearchParamsInit) => void;
}) {
  const [typed, setTyped] = useState(params.get("search") ?? "");

  const set = (field: string, value: string) => {
    const next = new URLSearchParams(params);
    if (value) next.set(field, value);
    else next.delete(field);
    onChange(next);
  };

  return (
    <>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          set("search", typed.trim());
        }}
      >
        <Input
          className="w-56"
          placeholder="model, account, request id…"
          value={typed}
          onChange={(event) => setTyped(event.target.value)}
        />
      </form>
      <Select
        className="w-32"
        value={params.get("status") ?? ""}
        onChange={(event) => set("status", event.target.value)}
      >
        {STATUSES.map((entry) => (
          <option key={entry} value={entry}>
            {entry === "" ? "any status" : entry.replace("_", " ")}
          </option>
        ))}
      </Select>
      {[...params.keys()].length > 0 ? (
        <Button
          tone="quiet"
          size="small"
          onClick={() => {
            setTyped("");
            onChange(new URLSearchParams());
          }}
        >
          Clear
        </Button>
      ) : null}
    </>
  );
}
