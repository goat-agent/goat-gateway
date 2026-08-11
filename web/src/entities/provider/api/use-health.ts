import { query, useResource } from "@/shared/api";
import type { Health } from "../model/types";

export function useHealth(windowMs?: number) {
  const path = `/api/overview${query({ window_ms: windowMs })}`;
  return useResource<Health>(path, [path]);
}
