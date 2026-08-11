import { useEffect, useRef } from "react";

export type Happening =
  | { happened: "request_opened"; id: string; provider: string; account: string | null; model: string }
  | { happened: "request_settled"; id: string; status: string; duration_ms: number | null; cost_micros: number | null }
  | { happened: "account_changed"; account: string; state: string; until: number | null }
  | { happened: "limits_observed"; account: string };

export function useHappenings(listen: (happening: Happening) => void) {
  const held = useRef(listen);
  held.current = listen;

  useEffect(() => {
    const source = new EventSource("/api/events");
    source.onmessage = (message) => {
      try {
        held.current(JSON.parse(message.data) as Happening);
      } catch {
        // a frame we cannot read is not worth tearing the stream down for
      }
    };
    return () => source.close();
  }, []);
}
