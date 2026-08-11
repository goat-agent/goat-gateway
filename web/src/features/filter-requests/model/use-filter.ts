import { useSearchParams } from "react-router-dom";

export function useFilter(fields: readonly string[]) {
  const [params, setParams] = useSearchParams();

  const held: Record<string, string> = {};
  for (const field of fields) {
    const value = params.get(field);
    if (value) held[field] = value;
  }

  const set = (field: string, value: string) => {
    const next = new URLSearchParams(params);
    if (value) next.set(field, value);
    else next.delete(field);
    setParams(next);
  };

  return { held, set, clear: () => setParams(new URLSearchParams()) };
}
