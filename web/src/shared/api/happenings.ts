import { useEffect, useRef } from "react";

const KINDS = ["request_opened", "request_settled", "account_changed", "limits_observed"] as const;

export type Happening = (typeof KINDS)[number];

type Listener = (happening: Happening) => void;

const listeners = new Set<Listener>();
let source: EventSource | undefined;

function subscribe(listener: Listener) {
  listeners.add(listener);
  if (!source) {
    source = new EventSource("/api/events");
    source.onmessage = (message) => {
      const happening = read(message.data);
      if (!happening) return;
      for (const each of listeners) each(happening);
    };
  }
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0 && source) {
      source.close();
      source = undefined;
    }
  };
}

export function useHappenings(listen: Listener) {
  const held = useRef(listen);
  held.current = listen;

  useEffect(() => subscribe((happening) => held.current(happening)), []);
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
