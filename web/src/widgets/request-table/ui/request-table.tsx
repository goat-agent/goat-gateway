import { Link } from "react-router-dom";
import { Body, Cell, Count, Head, Row, Table } from "@/shared/ui";
import { StatusBadge, type Request } from "@/entities/request";
import { duration, moment, money, tokens } from "@/shared/lib/format";

export function RequestTable({ requests, now }: { requests: Request[]; now: number }) {
  return (
    <Table>
      <Head>
        <Row>
          <th>Started</th>
          <th>Status</th>
          <th>Model</th>
          <th>Account</th>
          <th>Client</th>
          <Count>In</Count>
          <Count>Out</Count>
          <Count>Cost</Count>
          <Count>Took</Count>
        </Row>
      </Head>
      <Body>
        {requests.map((request) => (
          <Row key={request.id} className="hover:bg-raised">
            <td className="numeric">
              <Link
                to={`/requests/${request.id}`}
                className="text-ink-secondary no-underline hover:underline"
              >
                {moment(request.started_at, now)}
              </Link>
            </td>
            <td>
              <StatusBadge
                status={request.status}
                kind={request.error_kind}
                translated={request.translated}
              />
            </td>
            <td className="text-ink">{request.model}</td>
            <td className="text-ink-secondary">{request.account ?? "—"}</td>
            <td className="text-ink-secondary">{request.client ?? "—"}</td>
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
