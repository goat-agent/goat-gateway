import { Body, Cell, Count, Head, Row, Table, rowHover } from "@/shared/ui";
import { StatusGlyph, formatOf, type Request } from "@/entities/request";
import { useKeyNames } from "@/entities/user";
import { duration, moment, money, stamp, tokens, UNKNOWN } from "@/shared/lib/format";

const NARROW = "hidden @3xl:table-cell";
const MIDDLE = "hidden @4xl:table-cell";
const WIDE = "hidden @5xl:table-cell";

export function RequestTable({
  requests,
  now,
  selected,
  onSelect,
}: {
  requests: Request[];
  now: number;
  selected: string | undefined;
  onSelect: (id: string) => void;
}) {
  const keyName = useKeyNames();
  return (
    <Table sticky className="table-fixed">
      <Head>
        <Row>
          <th className="w-[46px]" aria-label="Status" />
          <th className="w-28">Started</th>
          <th>Model</th>
          <th className="w-40">Account</th>
          <th className={`w-32 ${MIDDLE}`}>Key</th>
          <th className={`w-40 ${WIDE}`}>Format</th>
          <Count className={`w-18 ${NARROW}`}>In</Count>
          <Count className={`w-18 ${NARROW}`}>Out</Count>
          <Count className={`w-20 ${MIDDLE}`}>Cost</Count>
          <Count className="w-20">Took</Count>
        </Row>
      </Head>
      <Body>
        {requests.map((request) => (
          <Row
            key={request.id}
            data-row={request.id}
            selected={request.id === selected}
            className={rowHover}
            onClick={() => onSelect(request.id)}
          >
            <td>
              <StatusGlyph status={request.status} kind={request.error_kind} />
            </td>
            <td className="mono truncate text-ink-secondary" title={moment(request.started_at, now)}>
              {stamp(request.started_at, now)}
            </td>
            <td className="truncate">
              <span className="font-medium text-ink">{request.model}</span>
              {request.status === "error" ? (
                <span className="ml-2 text-mini text-critical-text">
                  {request.error_kind ?? "failed"}
                </span>
              ) : null}
            </td>
            <td className="truncate text-ink-secondary">
              {request.credential === "carried" ? "own login" : (request.account ?? UNKNOWN)}
            </td>
            <td className={`truncate text-ink-secondary ${MIDDLE}`}>{keyName(request.key_id) || UNKNOWN}</td>
            <td
              className={`truncate text-mini text-ink-secondary ${WIDE}`}
              title={request.translated ? "the format was translated on the way through" : undefined}
            >
              {formatOf(request)}
            </td>
            <Cell className={NARROW}>{tokens(request.usage.input_tokens)}</Cell>
            <Cell className={NARROW}>{tokens(request.usage.output_tokens)}</Cell>
            <Cell className={MIDDLE}>{money(request.cost_micros)}</Cell>
            <Cell className="text-ink-secondary">{duration(request.duration_ms)}</Cell>
          </Row>
        ))}
      </Body>
    </Table>
  );
}
