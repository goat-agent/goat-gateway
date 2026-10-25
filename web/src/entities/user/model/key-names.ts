import { useResource } from "@/shared/api";
import type { Key } from "./types";

export function useKeyNames(): (id: string | null) => string {
  const listed = useResource<{ keys: Key[] }>("/api/keys").data?.keys ?? [];
  return (id) => listed.find((key) => key.id === id)?.label ?? id ?? "";
}
