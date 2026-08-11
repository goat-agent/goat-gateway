import { useCallback, useEffect, useState } from "react";
import { ask } from "./client";

export type Resource<T> = {
  data: T | undefined;
  error: string | undefined;
  loading: boolean;
  reload: () => void;
};

export function useResource<T>(path: string | null, deps: unknown[] = []): Resource<T> {
  const [data, setData] = useState<T>();
  const [error, setError] = useState<string>();
  const [loading, setLoading] = useState(path !== null);
  const [attempt, setAttempt] = useState(0);

  const reload = useCallback(() => setAttempt((count) => count + 1), []);

  useEffect(() => {
    if (path === null) return;
    let current = true;
    setLoading(true);

    ask<T>(path)
      .then((value) => {
        if (!current) return;
        setData(value);
        setError(undefined);
      })
      .catch((failure: Error) => {
        if (current) setError(failure.message);
      })
      .finally(() => {
        if (current) setLoading(false);
      });

    return () => {
      current = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [path, attempt, ...deps]);

  return { data, error, loading, reload };
}
