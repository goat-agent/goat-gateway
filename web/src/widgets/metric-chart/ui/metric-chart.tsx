import { scaleBand, scaleLinear } from "d3-scale";
import { useMemo, useState } from "react";
import type { Bucket, Metric, Slice } from "@/entities/usage";
import { seriesColor } from "@/shared/model";
import { show } from "@/shared/lib";

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

  const stacks = useMemo(() => stack(buckets, metric), [buckets, metric]);
  const tallest = Math.max(1, ...stacks.map((entry) => total(entry.parts)));

  const across = scaleBand<number>()
    .domain(stacks.map((entry) => entry.at))
    .range([GUTTER, 1000])
    .padding(0.28);
  const up = scaleLinear()
    .domain([0, tallest])
    .range([HEIGHT - 18, 4])
    .nice();

  if (stacks.length === 0) {
    return (
      <p className="px-3 py-14 text-center text-small text-ink-muted">
        Nothing has come through this gateway in this window yet.
      </p>
    );
  }

  const shown = stacks.find((entry) => entry.at === hovered);
  const wide = bucketMs >= 86_400_000;
  const when = (at: number) => (wide ? show.day(at) : show.clock(at));

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

        {stacks.map((entry) => {
          let base = up(0);
          return (
            <g
              key={entry.at}
              onMouseEnter={() => setHovered(entry.at)}
              onMouseLeave={() => setHovered(null)}
            >
              <rect
                x={across(entry.at) ?? 0}
                y={0}
                width={across.bandwidth()}
                height={HEIGHT}
                fill={hovered === entry.at ? "var(--line-subtle)" : "transparent"}
              />
              {entry.parts.map((part) => {
                const height = up(0) - up(part.value);
                base -= height;
                return (
                  <rect
                    key={part.key}
                    x={across(entry.at) ?? 0}
                    y={base}
                    width={across.bandwidth()}
                    height={Math.max(height, 1)}
                    fill={seriesColor(part.key)}
                    opacity={hovered === null || hovered === entry.at ? 1 : 0.45}
                  />
                );
              })}
            </g>
          );
        })}
      </svg>

      <figcaption className="flex h-5 items-center justify-between px-3 text-micro text-ink-muted">
        <span>{when(stacks[0]!.at)}</span>
        <span className="text-ink-secondary">
          {shown
            ? `${when(shown.at)} · ${metric.show(total(shown.parts))}`
            : `${metric.label.toLowerCase()}, ${stacks.length} buckets`}
        </span>
        <span>now</span>
      </figcaption>
    </figure>
  );
}

function stack(buckets: Bucket[], metric: Metric) {
  const keys = [...new Set(buckets.flatMap((bucket) => bucket.slices.map((slice) => slice.key)))].sort();
  return buckets.map((bucket) => ({
    at: bucket.at,
    parts: keys
      .map((key) => ({ key, value: valueOf(bucket.slices, key, metric) }))
      .filter((part) => part.value > 0),
  }));
}

function valueOf(slices: Slice[], key: string, metric: Metric) {
  const found = slices.find((slice) => slice.key === key);
  return found ? metric.of(found.totals) : 0;
}

function total(parts: { value: number }[]) {
  return parts.reduce((sum, part) => sum + part.value, 0);
}
