import type { Slice } from "@/entities/types";
import type { Metric } from "@/entities/metric";
import { seriesColor } from "@/shared/lib/series-color";

export function UsageBars({
  slices,
  metric,
  onPick,
}: {
  slices: Slice[];
  metric: Metric;
  onPick?: (key: string) => void;
}) {
  const rows = slices
    .map((slice) => ({ key: slice.key, value: metric.of(slice.totals) }))
    .filter((row) => row.value > 0)
    .sort((left, right) => right.value - left.value);

  const widest = Math.max(1, ...rows.map((row) => row.value));

  if (rows.length === 0) {
    return (
      <p className="px-3 py-8 text-center text-small text-ink-muted">
        No {metric.label.toLowerCase()} to split up yet.
      </p>
    );
  }

  return (
    <ul className="m-0 flex list-none flex-col gap-2 p-3">
      {rows.map((row) => (
        <li key={row.key}>
          <button
            type="button"
            disabled={!onPick}
            onClick={() => onPick?.(row.key)}
            className="group flex w-full flex-col gap-1 text-left disabled:cursor-default"
          >
            <span className="flex items-baseline justify-between gap-3 text-small">
              <span className="truncate text-ink group-enabled:group-hover:underline">
                {row.key || "unattributed"}
              </span>
              <span className="numeric shrink-0 text-ink-secondary">{metric.show(row.value)}</span>
            </span>
            <span className="block h-1.5 w-full rounded-full bg-raised">
              <span
                className="block h-full rounded-full"
                style={{
                  width: `${Math.max(2, (row.value / widest) * 100)}%`,
                  background: seriesColor(row.key),
                }}
              />
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}
