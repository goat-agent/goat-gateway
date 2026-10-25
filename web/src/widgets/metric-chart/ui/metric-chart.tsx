import { scaleBand, scaleLinear } from "d3-scale";
import { useMemo, useState } from "react";
import type { Bucket, Metric, Slice } from "@/entities/usage";
import { seriesColor } from "@/shared/model/series";
import { clock, day } from "@/shared/lib/format";
import { useWidth } from "@/shared/lib/use-width";

const HEIGHT = 184;
const GUTTER = 52;
const TOP = 10;
const BOTTOM = 6;
const GAP = 2;
const ROUNDING = 3;
const READOUT = 208;
const WIDEST = 40;

type Part = { key: string; value: number };
type Stack = { at: number; parts: Part[] };

export function MetricChart({
  buckets,
  metric,
  bucketMs,
  windowMs,
  space,
}: {
  buckets: Bucket[];
  metric: Metric;
  bucketMs: number;
  windowMs: number;
  space: string;
}) {
  const { held, width } = useWidth<HTMLDivElement>();
  const [hovered, setHovered] = useState<number>();

  const stacks = useMemo(
    () => stacked(filled(buckets, bucketMs, windowMs, Date.now()), metric),
    [buckets, bucketMs, windowMs, metric],
  );
  const [first] = stacks;
  const silent = stacks.every((stack) => stack.parts.length === 0);

  const tallest = Math.max(1, ...stacks.map((stack) => added(stack.parts)));
  const across = scaleBand<number>()
    .domain(stacks.map((stack) => stack.at))
    .range([GUTTER, Math.max(GUTTER + 1, width)])
    .paddingInner(0.32)
    .paddingOuter(0.12);
  const up = scaleLinear()
    .domain([0, tallest])
    .range([HEIGHT - BOTTOM, TOP])
    .nice();

  const shown = stacks.find((stack) => stack.at === hovered);
  const wide = bucketMs >= 86_400_000;
  const step = across.step();
  const bar = Math.min(across.bandwidth(), WIDEST);
  const inset = (across.bandwidth() - bar) / 2;
  const ticks = labelled(up.ticks(4), metric);

  return (
    <figure className="m-0 px-5 pb-2">
      <div ref={held} className="relative" onMouseLeave={() => setHovered(undefined)}>
        {first && !silent && width > 0 ? (
          <svg
            width={width}
            height={HEIGHT}
            role="img"
            aria-label={`${metric.label} over time`}
            className="block overflow-visible"
          >
            {ticks.map(({ tick, label }) => (
              <g key={tick}>
                <line
                  x1={GUTTER}
                  x2={width}
                  y1={Math.round(up(tick)) + 0.5}
                  y2={Math.round(up(tick)) + 0.5}
                  stroke="var(--line-subtle)"
                  strokeWidth={1}
                />
                <text
                  x={GUTTER - 10}
                  y={up(tick)}
                  dy="0.32em"
                  textAnchor="end"
                  className="numeric fill-[var(--text-secondary)] text-micro"
                >
                  {label}
                </text>
              </g>
            ))}

            {stacks.map((stack) => {
              const x = (across(stack.at) ?? 0) + inset;
              const pieces = piled(stack.parts, up);
              const dimmed = hovered !== undefined && hovered !== stack.at;
              return (
                <g key={stack.at} onMouseEnter={() => setHovered(stack.at)}>
                  <rect
                    x={x - inset - (step - across.bandwidth()) / 2}
                    y={0}
                    width={step}
                    height={HEIGHT}
                    rx={4}
                    fill={hovered === stack.at ? "var(--bg-panel-hover)" : "transparent"}
                  />
                  {pieces.map((piece, index) =>
                    index === pieces.length - 1 ? (
                      <path
                        key={piece.key}
                        d={roundedTop(x, piece.top, bar, piece.height)}
                        fill={seriesColor(space, piece.key)}
                        opacity={dimmed ? 0.4 : 1}
                      />
                    ) : (
                      <rect
                        key={piece.key}
                        x={x}
                        y={piece.top}
                        width={bar}
                        height={piece.height}
                        fill={seriesColor(space, piece.key)}
                        opacity={dimmed ? 0.4 : 1}
                      />
                    ),
                  )}
                </g>
              );
            })}

            <line
              x1={GUTTER}
              x2={width}
              y1={Math.round(up(0)) + 0.5}
              y2={Math.round(up(0)) + 0.5}
              stroke="var(--line)"
              strokeWidth={1}
            />
          </svg>
        ) : null}

        {!first || silent ? (
          <p className="m-0 flex h-[184px] items-center justify-center text-small text-ink-secondary">
            {first
              ? `Nothing in this window reports ${metric.label.toLowerCase()}.`
              : "Nothing has come through this gateway in this window yet."}
          </p>
        ) : null}

        {shown && width > 0 ? (
          <Readout
            stack={shown}
            metric={metric}
            space={space}
            heading={when(shown.at, wide)}
            left={clamped((across(shown.at) ?? 0) + across.bandwidth() / 2, width)}
          />
        ) : null}
      </div>

      {first && !silent ? (
        <figcaption
          className="numeric flex h-6 items-center justify-between text-micro text-ink-secondary"
          style={{ paddingLeft: GUTTER }}
        >
          <span>{when(first.at, wide)}</span>
          <span className="text-ink-muted">
            {metric.label.toLowerCase()}, {stacks.length} buckets
          </span>
          <span>now</span>
        </figcaption>
      ) : null}
    </figure>
  );
}

function Readout({
  stack,
  metric,
  space,
  heading,
  left,
}: {
  stack: Stack;
  metric: Metric;
  space: string;
  heading: string;
  left: number;
}) {
  const parts = [...stack.parts].sort((one, other) => other.value - one.value);

  return (
    <div
      className="pointer-events-none absolute top-1 z-10 flex flex-col gap-1 rounded-lg hairline border-line-menu bg-menu px-3 py-2 text-mini shadow-medium"
      style={{ left, width: READOUT, transform: "translateX(-50%)" }}
    >
      <span className="numeric text-ink-secondary">{heading}</span>
      {parts.length === 0 ? (
        <span className="text-ink-secondary">nothing in this bucket</span>
      ) : (
        parts.map((part) => (
          <span key={part.key} className="flex items-center gap-2">
            <span
              className="size-2 shrink-0 rounded-[2px]"
              style={{ background: seriesColor(space, part.key) }}
            />
            <span className="min-w-0 grow truncate text-ink">{part.key || "unattributed"}</span>
            <span className="numeric text-ink">{metric.show(part.value)}</span>
          </span>
        ))
      )}
      {parts.length > 1 ? (
        <span className="mt-0.5 flex items-center justify-between hairline-t border-line-menu pt-1 font-medium text-ink-title">
          <span>Total</span>
          <span className="numeric">{metric.show(added(parts))}</span>
        </span>
      ) : null}
    </div>
  );
}

function filled(buckets: Bucket[], bucketMs: number, windowMs: number, now: number): Bucket[] {
  const [anchor] = buckets;
  if (!anchor || bucketMs <= 0) return buckets;

  const phase = ((anchor.at % bucketMs) + bucketMs) % bucketMs;
  const latest = now - ((((now - phase) % bucketMs) + bucketMs) % bucketMs);
  const count = Math.max(1, Math.ceil(windowMs / bucketMs));

  const held = new Map(buckets.map((bucket) => [bucket.at, bucket]));
  for (let index = 0; index < count; index += 1) {
    const at = latest - index * bucketMs;
    if (!held.has(at)) held.set(at, { at, slices: [] });
  }
  return [...held.values()].sort((one, other) => one.at - other.at);
}

function labelled(ticks: number[], metric: Metric) {
  const seen = new Set<string>();
  return ticks
    .filter((tick) => Number.isInteger(tick))
    .map((tick) => ({ tick, label: metric.show(tick) }))
    .filter(({ label }) => {
      if (seen.has(label)) return false;
      seen.add(label);
      return true;
    });
}

function stacked(buckets: Bucket[], metric: Metric): Stack[] {
  const keys = [
    ...new Set(buckets.flatMap((bucket) => bucket.slices.map((slice) => slice.key))),
  ].sort();
  return buckets.map((bucket) => ({
    at: bucket.at,
    parts: keys
      .map((key) => ({ key, value: valueOf(bucket.slices, key, metric) }))
      .filter((part) => part.value > 0),
  }));
}

function piled(parts: Part[], up: (value: number) => number) {
  const floor = up(0);
  let base = floor;
  return parts.map((part, index) => {
    const full = Math.max(floor - up(part.value), 1);
    const top = base - full;
    const bottom = index === 0 ? base : base - GAP;
    base = top;
    return { key: part.key, top, height: Math.max(bottom - top, 1) };
  });
}

function roundedTop(x: number, y: number, width: number, height: number) {
  const radius = Math.min(ROUNDING, width / 2, height);
  return [
    `M${x},${y + height}`,
    `V${y + radius}`,
    `Q${x},${y} ${x + radius},${y}`,
    `H${x + width - radius}`,
    `Q${x + width},${y} ${x + width},${y + radius}`,
    `V${y + height}`,
    "Z",
  ].join(" ");
}

function clamped(center: number, width: number) {
  const half = READOUT / 2;
  return Math.min(Math.max(center, half), Math.max(half, width - half));
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
