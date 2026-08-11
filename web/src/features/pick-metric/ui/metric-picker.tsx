import { Button } from "@/shared/ui";
import { METRICS, METRIC_NAMES, type MetricName } from "@/entities/usage";

export function MetricPicker({
  chosen,
  onPick,
}: {
  chosen: MetricName;
  onPick: (name: MetricName) => void;
}) {
  return (
    <>
      {METRIC_NAMES.map((name) => (
        <Button
          key={name}
          size="small"
          tone={name === chosen ? "plain" : "quiet"}
          onClick={() => onPick(name)}
        >
          {METRICS[name].label}
        </Button>
      ))}
    </>
  );
}
