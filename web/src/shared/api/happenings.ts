import { useEffect, useRef } from "react";

const KINDS = ["request_opened", "request_settled", "account_changed", "limits_observed"] as const;

export type Happening = (typeof KINDS)[number];

export function useHappenings(listen: (happening: Happening) => void) {
  const held = useRef(listen);
  held.current = listen;

  useEffect(() => {
    const source = new EventSource("/api/events");
    source.onmessage = (message) => {
      const happening = read(message.data);
      if (happening) held.current(happening);
    };
    return () => source.close();
  }, []);
}

function read(data: unknown): Happening | undefined {
  if (typeof data !== "string") return undefined;
  const body = parsed(data);
  if (typeof body !== "object" || body === null || !("happened" in body)) return undefined;
  return KINDS.find((kind) => kind === body.happened);
}

function parsed(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}
