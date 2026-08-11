import { query, useResource } from "@/shared/api";
import type { Grouping, Report } from "./types";

export type Asked = {
  since: number;
  by: Grouping;
  bucketMs: number;
  narrowed?: Record<string, string>;
};

export function useReport({ since, by, bucketMs, narrowed }: Asked) {
  const path = `/api/usage${query({ since, by, bucket_ms: bucketMs, ...narrowed })}`;
  return useResource<Report>(path, [path]);
}
