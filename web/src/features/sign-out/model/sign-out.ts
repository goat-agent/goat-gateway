import { drop } from "@/shared/api";

export function signOut() {
  void drop("/api/session").finally(() => window.location.reload());
}
