import { Link, useNavigate } from "react-router-dom";
import { Body, Cell, Count, Head, Pill, Row, Table, rowHover } from "@/shared/ui";
import { StatusBadge, type Request } from "@/entities/request";
import { duration, moment, money, tokens, UNKNOWN } from "@/shared/lib/format";

export function RequestTable({ requests, now }: { requests: Request[]; now: number }) {
  const navigate = useNavigate();

  return (
    <Table contained={false} className="[&_td:first-child]:pl-5 [&_td:last-child]:pr-5 [&_th:first-child]:pl-5 [&_th:last-child]:pr-5 [&_thead_th]:sticky [&_thead_th]:top-0 [&_thead_th]:z-[1] [&_thead_th]:border-t-0 [&_thead_th]:bg-panel">
      <Head>
        <Row>
          <th className="w-32">Status</th>
          <th className="w-36">Started</th>
          <th>Model</th>
          <th>Account</th>
          <th>Client</th>
          <th>Format</th>
          <Count>In</Count>
          <Count>Out</Count>
          <Count>Cost</Count>
          <Count>Took</Count>
        </Row>
      </Head>
      <Body>
        {requests.map((request) => (
          <Row
            key={request.id}
            className={rowHover}
            onClick={() => navigate(`/requests/${request.id}`)}
          >
            <td>
              <StatusBadge status={request.status} kind={request.error_kind} />
            </td>
            <td>
              <Link
                to={`/requests/${request.id}`}
                className="mono text-ink-secondary hover:text-ink"
                onClick={(event) => event.stopPropagation()}
              >
                {moment(request.started_at, now)}
              </Link>
            </td>
            <td className="max-w-72 truncate font-medium text-ink">{request.model}</td>
            <td className="max-w-48 truncate text-ink-secondary">{request.account ?? UNKNOWN}</td>
            <td className="max-w-40 truncate text-ink-secondary">{request.client ?? UNKNOWN}</td>
            <td>
              <Pill
                className="h-5 px-2 text-micro text-ink-secondary"
                title={request.translated ? "the format was translated on the way through" : undefined}
              >
                {request.translated ? `${request.ingress} → ${request.egress}` : request.ingress}
              </Pill>
            </td>
            <Cell>{tokens(request.usage.input_tokens)}</Cell>
            <Cell>{tokens(request.usage.output_tokens)}</Cell>
            <Cell>{money(request.cost_micros)}</Cell>
            <Cell className="text-ink-secondary">{duration(request.duration_ms)}</Cell>
          </Row>
        ))}
      </Body>
    </Table>
  );
}
