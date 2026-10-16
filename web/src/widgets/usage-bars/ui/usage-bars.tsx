import type { Metric, Slice } from "@/entities/usage";
import { seriesColor } from "@/shared/model/series";

export function UsageBars({
  slices,
  metric,
  space,
  onPick,
}: {
  slices: Slice[];
  metric: Metric;
  space: string;
  onPick?: (key: string) => void;
}) {
  const rows = slices
    .map((slice) => ({ key: slice.key, value: metric.of(slice.totals) }))
    .filter((row) => row.value > 0)
    .sort((left, right) => right.value - left.value);

  const widest = Math.max(1, ...rows.map((row) => row.value));

  if (rows.length === 0) {
    return (
      <p className="m-0 px-4 py-10 text-center text-small text-ink-secondary">
        Nothing here reports {metric.label.toLowerCase()}.
      </p>
    );
  }

  return (
    <ul className="m-0 flex list-none flex-col gap-0.5 px-2 pb-2">
      {rows.map((row) => (
        <li key={row.key}>
          <button
            type="button"
            disabled={!onPick}
            title={onPick ? `Only show ${row.key || "unattributed"}` : undefined}
            onClick={() => onPick?.(row.key)}
            className="hover-fade flex w-full flex-col gap-1.5 rounded-md px-2 py-2 text-left enabled:hover:bg-panel-hover"
          >
            <span className="flex w-full items-baseline justify-between gap-3 text-small">
              <span className="min-w-0 truncate text-ink">{row.key || "unattributed"}</span>
              <span className="numeric shrink-0 text-ink-secondary">{metric.show(row.value)}</span>
            </span>
            <span className="block h-1 w-full rounded-full bg-line-subtle">
              <span
                className="block h-full rounded-full"
                style={{
                  width: `${Math.max(2, (row.value / widest) * 100)}%`,
                  background: seriesColor(space, row.key),
                }}
              />
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}
