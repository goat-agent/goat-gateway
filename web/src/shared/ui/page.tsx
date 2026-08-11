import type { ReactNode } from "react";

export function Page({
  title,
  note,
  aside,
  children,
}: {
  title: ReactNode;
  note?: ReactNode;
  aside?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-4">
      <header className="flex min-h-8 flex-wrap items-center justify-between gap-3">
        <div className="flex items-baseline gap-2.5">
          <h1 className="m-0 text-heading font-semibold tracking-tight">{title}</h1>
          {note ? <span className="text-small text-ink-muted">{note}</span> : null}
        </div>
        {aside ? <div className="flex items-center gap-2">{aside}</div> : null}
      </header>
      {children}
    </div>
  );
}

export function Tile({
  label,
  value,
  note,
  tone,
}: {
  label: string;
  value: ReactNode;
  note?: ReactNode;
  tone?: "good" | "warning" | "critical";
}) {
  return (
    <div className="flex flex-col gap-1 rounded-md border border-line-subtle bg-panel px-3 py-2.5">
      <span className="text-micro tracking-wide text-ink-muted uppercase">{label}</span>
      <span className="numeric text-[19px] leading-tight" style={{ color: ink(tone) }}>
        {value}
      </span>
      {note ? <span className="text-micro text-ink-muted">{note}</span> : null}
    </div>
  );
}

function ink(tone: "good" | "warning" | "critical" | undefined) {
  switch (tone) {
    case "good":
      return "var(--good)";
    case "warning":
      return "var(--warning)";
    case "critical":
      return "var(--critical)";
    default:
      return "var(--text)";
  }
}

export function Fact({ name, value }: { name: string; value: ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5 border-b border-line-subtle py-2">
      <dt className="text-micro tracking-wide text-ink-muted uppercase">{name}</dt>
      <dd className="numeric m-0 truncate text-ink">{value}</dd>
    </div>
  );
}

export function Facts({ children }: { children: ReactNode }) {
  return <dl className="m-0 grid grid-cols-2 gap-x-6 gap-y-0 p-3 sm:grid-cols-3">{children}</dl>;
}
