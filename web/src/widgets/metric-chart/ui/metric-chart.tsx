import { scaleBand, scaleLinear } from "d3-scale";
import { useMemo, useState } from "react";
import type { Bucket, Metric, Slice } from "@/entities/usage";
import { seriesColor } from "@/shared/model/series";
import { clock, day } from "@/shared/lib/format";

const HEIGHT = 168;
const GUTTER = 44;
const WIDTH = 1000;

type Part = { key: string; value: number };
type Stack = { at: number; parts: Part[] };

export function MetricChart({
  buckets,
  metric,
  bucketMs,
}: {
  buckets: Bucket[];
  metric: Metric;
  bucketMs: number;
}) {
  const [hovered, setHovered] = useState<number>();

  const stacks = useMemo(() => stacked(buckets, metric), [buckets, metric]);
  const [first] = stacks;

  const tallest = Math.max(1, ...stacks.map((stack) => added(stack.parts)));
  const across = scaleBand<number>()
    .domain(stacks.map((stack) => stack.at))
    .range([GUTTER, WIDTH])
    .padding(0.28);
  const up = scaleLinear().domain([0, tallest]).range([HEIGHT - 18, 4]).nice();

  if (!first) {
    return (
      <p className="m-0 px-3 py-10 text-center text-small text-ink-muted">
        Nothing has come through this gateway in this window yet.
      </p>
    );
  }

  const shown = stacks.find((stack) => stack.at === hovered);
  const wide = bucketMs >= 86_400_000;

  return (
    <figure className="m-0">
      <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} className="h-[168px] w-full" preserveAspectRatio="none">
        {up.ticks(4).map((tick) => (
          <g key={tick}>
            <line
              x1={GUTTER}
              x2={WIDTH}
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

        {stacks.map((stack) => (
          <g
            key={stack.at}
            onMouseEnter={() => setHovered(stack.at)}
            onMouseLeave={() => setHovered(undefined)}
          >
            <rect
              x={across(stack.at) ?? 0}
              y={0}
              width={across.bandwidth()}
              height={HEIGHT}
              fill={hovered === stack.at ? "var(--line-subtle)" : "transparent"}
            />
            {piled(stack.parts, up).map((piece) => (
              <rect
                key={piece.key}
                x={across(stack.at) ?? 0}
                y={piece.top}
                width={across.bandwidth()}
                height={piece.height}
                fill={seriesColor(piece.key)}
                opacity={hovered === undefined || hovered === stack.at ? 1 : 0.45}
              />
            ))}
          </g>
        ))}
      </svg>

      <figcaption className="flex h-5 items-center justify-between px-3 text-micro text-ink-muted">
        <span>{when(first.at, wide)}</span>
        <span className="text-ink-secondary">
          {shown
            ? `${when(shown.at, wide)} · ${metric.show(added(shown.parts))}`
            : `${metric.label.toLowerCase()}, ${stacks.length} buckets`}
        </span>
        <span>now</span>
      </figcaption>
    </figure>
  );
}

function stacked(buckets: Bucket[], metric: Metric): Stack[] {
  const keys = [...new Set(buckets.flatMap((bucket) => bucket.slices.map((slice) => slice.key)))].sort();
  return buckets.map((bucket) => ({
    at: bucket.at,
    parts: keys
      .map((key) => ({ key, value: valueOf(bucket.slices, key, metric) }))
      .filter((part) => part.value > 0),
  }));
}

function piled(parts: Part[], up: (value: number) => number) {
  let base = up(0);
  return parts.map((part) => {
    const height = Math.max(up(0) - up(part.value), 1);
    base -= height;
    return { key: part.key, top: base, height };
  });
}

function when(at: number, wide: boolean) {
  return wide ? day(at) : clock(at);
}

function valueOf(slices: Slice[], key: string, metric: Metric) {
  const found = slices.find((slice) => slice.key === key);
  return found ? metric.of(found.totals) : 0;
}

function added(parts: Part[]) {
  return parts.reduce((total, part) => total + part.value, 0);
}
