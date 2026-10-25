import { useEffect } from "react";
import { tinykeys } from "tinykeys";
import { typingOrOverlaid } from "./keys";

const SEQUENCE_MS = 1000;

export function useRowKeys({
  ids,
  selected,
  select,
  close,
}: {
  ids: readonly string[];
  selected: string | undefined;
  select: (id: string) => void;
  close: () => void;
}) {
  const order = ids.join("\n");

  useEffect(() => {
    const rows = order === "" ? [] : order.split("\n");
    let going = 0;
    const step = (by: number) => (event: KeyboardEvent) => {
      if (rows.length === 0 || event.timeStamp - going < SEQUENCE_MS) return;
      event.preventDefault();
      const at = selected === undefined ? -1 : rows.indexOf(selected);
      const next = at === -1 ? (by > 0 ? 0 : rows.length - 1) : at + by;
      const landed = rows[Math.min(rows.length - 1, Math.max(0, next))];
      if (landed !== undefined && landed !== selected) select(landed);
    };

    return tinykeys(
      window,
      {
        g: (event) => {
          going = event.timeStamp;
        },
        j: step(1),
        k: step(-1),
        ArrowDown: step(1),
        ArrowUp: step(-1),
        Escape: (event) => {
          if (selected === undefined) return;
          event.preventDefault();
          close();
        },
      },
      { ignore: typingOrOverlaid },
    );
  }, [order, selected, select, close]);

  useEffect(() => {
    if (selected === undefined) return;
    document
      .querySelector(`[data-row="${CSS.escape(selected)}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [selected]);
}
