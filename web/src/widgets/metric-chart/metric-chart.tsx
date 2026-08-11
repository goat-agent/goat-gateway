import { scaleBand, scaleLinear } from "d3-scale";
import { useMemo, useState } from "react";
import type { Bucket, Slice } from "@/entities/types";
import type { Metric } from "@/entities/metric";
import { seriesColor } from "@/shared/lib/series-color";
import { clock, day } from "@/shared/lib/format";

const HEIGHT = 168;
const GUTTER = 44;

export function MetricChart({
  buckets,
  metric,
  bucketMs,
}: {
  buckets: Bucket[];
  metric: Metric;
  bucketMs: number;
}) {
  const [hovered, setHovered] = useState<number | null>(null);

  const keys = useMemo(() => {
    const seen = new Set<string>();
    for (const bucket of buckets) for (const slice of bucket.slices) seen.add(slice.key);
    return [...seen].sort();
  }, [buckets]);

  const stacks = useMemo(
    () =>
      buckets.map((bucket) => ({
        at: bucket.at,
        parts: keys
          .map((key) => ({ key, value: valueOf(bucket.slices, key, metric) }))
          .filter((part) => part.value > 0),
      })),
    [buckets, keys, metric],
  );

  const tallest = Math.max(1, ...stacks.map((stack) => sum(stack.parts)));
  const across = scaleBand<number>()
    .domain(stacks.map((stack) => stack.at))
    .range([GUTTER, 1000])
    .padding(0.28);
  const up = scaleLinear().domain([0, tallest]).range([HEIGHT - 18, 4]).nice();

  if (stacks.length === 0) {
    return (
      <p className="px-3 py-10 text-center text-small text-ink-muted">
        Nothing has come through this gateway in this window yet.
      </p>
    );
  }

  const shown = hovered !== null ? stacks.find((stack) => stack.at === hovered) : undefined;
  const wide = bucketMs >= 86_400_000;

  return (
    <figure className="m-0">
      <svg viewBox={`0 0 1000 ${HEIGHT}`} className="h-[168px] w-full" preserveAspectRatio="none">
        {up.ticks(4).map((tick) => (
          <g key={tick}>
            <line
              x1={GUTTER}
              x2={1000}
              y1={up(tick)}
              y2={up(tick)}
              stroke="var(--line-subtle)"
              strokeWidth={1}
              vectorEffect="non-scaling-stroke"
            />
            <text
              x={GUTTER - 6}
              y={up(tick) + 3}
              textAnchor="end"
              className="fill-[var(--text-muted)] font-mono"
              style={{ fontSize: 9 }}
            >
              {metric.show(tick)}
            </text>
          </g>
        ))}

        {stacks.map((stack) => {
          let base = up(0);
          return (
            <g
              key={stack.at}
              onMouseEnter={() => setHovered(stack.at)}
              onMouseLeave={() => setHovered(null)}
            >
              <rect
                x={across(stack.at) ?? 0}
                y={0}
                width={across.bandwidth()}
                height={HEIGHT}
                fill={hovered === stack.at ? "var(--line-subtle)" : "transparent"}
              />
              {stack.parts.map((part) => {
                const height = up(0) - up(part.value);
                base -= height;
                return (
                  <rect
                    key={part.key}
                    x={across(stack.at) ?? 0}
                    y={base}
                    width={across.bandwidth()}
                    height={Math.max(height, 1)}
                    fill={seriesColor(part.key)}
                    opacity={hovered === null || hovered === stack.at ? 1 : 0.45}
                  />
                );
              })}
            </g>
          );
        })}
      </svg>

      <figcaption className="flex h-5 items-center justify-between px-3 text-micro text-ink-muted">
        <span>{when(stacks[0]!.at, wide)}</span>
        <span className="text-ink-secondary">
          {shown
            ? `${when(shown.at, wide)} · ${metric.show(sum(shown.parts))}`
            : `${metric.label.toLowerCase()}, ${stacks.length} buckets`}
        </span>
        <span>now</span>
      </figcaption>
    </figure>
  );
}

function when(at: number, wide: boolean) {
  return wide ? day(at) : clock(at);
}

function valueOf(slices: Slice[], key: string, metric: Metric) {
  const found = slices.find((slice) => slice.key === key);
  return found ? metric.of(found.totals) : 0;
}

function sum(parts: { value: number }[]) {
  return parts.reduce((total, part) => total + part.value, 0);
}
