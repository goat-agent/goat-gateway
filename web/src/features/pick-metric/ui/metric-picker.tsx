import { SegmentedControl } from "@/shared/ui";
import { METRICS, METRIC_NAMES, type MetricName } from "@/entities/usage";

const CHOICES = METRIC_NAMES.map((name) => ({ value: name, label: METRICS[name].label }));

export function MetricPicker({
  chosen,
  onPick,
}: {
  chosen: MetricName;
  onPick: (name: MetricName) => void;
}) {
  return <SegmentedControl label="Metric" value={chosen} choices={CHOICES} onChange={onPick} />;
}
