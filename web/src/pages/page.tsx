import type { ReactNode } from "react";

export function Page({
  title,
  note,
  children,
  aside,
}: {
  title: string;
  note?: ReactNode;
  children: ReactNode;
  aside?: ReactNode;
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
  const colour =
    tone === "critical"
      ? "var(--critical)"
      : tone === "warning"
        ? "var(--warning)"
        : tone === "good"
          ? "var(--good)"
          : "var(--text)";

  return (
    <div className="flex flex-col gap-1 rounded-md border border-line-subtle bg-panel px-3 py-2.5">
      <span className="text-micro tracking-wide text-ink-muted uppercase">{label}</span>
      <span className="numeric text-[19px] leading-tight" style={{ color: colour }}>
        {value}
      </span>
      {note ? <span className="text-micro text-ink-muted">{note}</span> : null}
    </div>
  );
}
