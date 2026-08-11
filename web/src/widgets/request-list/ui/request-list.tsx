import { Link } from "react-router-dom";
import { StatusBadge, type Request } from "@/entities/request";
import { Body, Head, Numeric, NumericHead, Row, Table } from "@/shared/ui";
import { show } from "@/shared/lib";

export function RequestList({ requests }: { requests: Request[] }) {
  const spread = spansMoreThanADay(requests);

  return (
    <Table>
      <Head>
        <Row>
          <th>Started</th>
          <th>Status</th>
          <th>Model</th>
          <th>Account</th>
          <th>Client</th>
          <NumericHead>In</NumericHead>
          <NumericHead>Out</NumericHead>
          <NumericHead>Cost</NumericHead>
          <NumericHead>Took</NumericHead>
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
                {spread
                  ? `${show.day(request.started_at)} ${show.clock(request.started_at)}`
                  : show.clock(request.started_at)}
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
            <td className="text-ink-secondary">{request.account ?? show.UNKNOWN}</td>
            <td className="text-ink-secondary">{request.client ?? show.UNKNOWN}</td>
            <Numeric>{show.tokens(request.usage.input_tokens)}</Numeric>
            <Numeric>{show.tokens(request.usage.output_tokens)}</Numeric>
            <Numeric>{show.money(request.cost_micros)}</Numeric>
            <Numeric className="text-ink-secondary">{show.duration(request.duration_ms)}</Numeric>
          </Row>
        ))}
      </Body>
    </Table>
  );
}

function spansMoreThanADay(requests: Request[]) {
  if (requests.length === 0) return false;
  const times = requests.map((request) => request.started_at);
  return Math.max(...times) - Math.min(...times) > 86_400_000;
}
