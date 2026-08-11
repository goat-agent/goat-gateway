import type { ReactNode } from "react";

export function Nothing({ says, offers }: { says: ReactNode; offers?: ReactNode }) {
  return (
    <div
      data-slot="nothing"
      className="flex flex-col items-center justify-center gap-3 px-6 py-14 text-center"
    >
      <p className="max-w-sm text-small text-ink-secondary">{says}</p>
      {offers}
    </div>
  );
}
