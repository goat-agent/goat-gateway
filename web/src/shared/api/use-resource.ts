import { useCallback, useEffect, useState } from "react";
import { ask } from "./client";

export type Resource<T> = {
  data: T | undefined;
  error: string | undefined;
  loading: boolean;
  reload: () => void;
};

export function useResource<T>(path: string | undefined): Resource<T> {
  const [data, setData] = useState<T>();
  const [error, setError] = useState<string>();
  const [loading, setLoading] = useState(path !== undefined);
  const [attempt, setAttempt] = useState(0);

  const reload = useCallback(() => setAttempt((count) => count + 1), []);

  useEffect(() => {
    if (path === undefined) return;
    let listening = true;
    setLoading(true);

    ask<T>(path)
      .then((value) => {
        if (!listening) return;
        setData(value);
        setError(undefined);
      })
      .catch((failure: Error) => {
        if (listening) setError(failure.message);
      })
      .finally(() => {
        if (listening) setLoading(false);
      });

    return () => {
      listening = false;
    };
  }, [path, attempt]);

  return { data, error, loading, reload };
}
