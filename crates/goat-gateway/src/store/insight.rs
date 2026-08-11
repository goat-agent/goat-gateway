use rusqlite::{params_from_iter, types::Value as Sql};
use serde::{Deserialize, Serialize};

use crate::store::{RequestRow, Store, StoreError, Usage, now};

pub const ABANDONED_AFTER_MS: i64 = 600_000;

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub provider: Option<String>,
    pub account: Option<String>,
    pub model: Option<String>,
    pub person: Option<String>,
    pub client: Option<String>,
    pub conversation: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
}

impl Filter {
    pub fn from_query(asked: &std::collections::HashMap<String, String>) -> Self {
        let text = |name: &str| asked.get(name).map(String::from).filter(|v| !v.is_empty());
        Self {
            since: number(asked, "since"),
            until: number(asked, "until"),
            provider: text("provider"),
            account: text("account"),
            model: text("model"),
            person: text("person"),
            client: text("client"),
            conversation: text("conversation"),
            status: text("status"),
            search: text("search"),
        }
    }

    pub fn since(window_ms: i64) -> Self {
        Self {
            since: Some(now() - window_ms),
            ..Self::default()
        }
    }

    fn sql(&self) -> (String, Vec<Sql>) {
        let mut clauses = Vec::new();
        let mut binds = Vec::new();
        let mut equals = |column: &str, value: &Option<String>| {
            if let Some(value) = value {
                clauses.push(format!("{column} = ?"));
                binds.push(Sql::Text(value.clone()));
            }
        };

        equals("provider", &self.provider);
        equals("account", &self.account);
        equals("model", &self.model);
        equals("person", &self.person);
        equals("client", &self.client);
        equals("conversation", &self.conversation);
        equals("status", &self.status);

        if let Some(since) = self.since {
            clauses.push("started_at >= ?".to_owned());
            binds.push(Sql::Integer(since));
        }
        if let Some(until) = self.until {
            clauses.push("started_at < ?".to_owned());
            binds.push(Sql::Integer(until));
        }
        if let Some(search) = self
            .search
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            const LOOKED_UP: &[&str] = &[
                "id",
                "model",
                "account",
                "error_kind",
                "upstream_request_id",
            ];
            clauses.push(format!(
                "({})",
                LOOKED_UP
                    .iter()
                    .map(|column| format!("{column} LIKE ?"))
                    .collect::<Vec<_>>()
                    .join(" OR ")
            ));
            let like = Sql::Text(format!("%{search}%"));
            binds.extend(std::iter::repeat_n(like, LOOKED_UP.len()));
        }

        let where_clause = if clauses.is_empty() {
            "1 = 1".to_owned()
        } else {
            clauses.join(" AND ")
        };
        (where_clause, binds)
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Totals {
    pub requests: i64,
    pub errors: i64,
    pub in_flight: i64,
    pub usage: Usage,
    pub cost_micros: Option<i64>,
    pub priced_requests: i64,
    pub median_ms: Option<i64>,
    pub slowest_tenth_ms: Option<i64>,
}

impl Totals {
    pub fn cache_hit_ratio(&self) -> Option<f64> {
        let read = self.usage.cache_read_tokens.unwrap_or(0) as f64;
        let fresh = self.usage.input_tokens.unwrap_or(0) as f64;
        (read + fresh > 0.0).then(|| read / (read + fresh))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Slice {
    pub key: String,
    pub totals: Totals,
}

pub fn number(asked: &std::collections::HashMap<String, String>, name: &str) -> Option<i64> {
    asked.get(name)?.parse().ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum By {
    Provider,
    Account,
    Model,
    Person,
    Client,
    Status,
}

impl By {
    pub fn named(text: Option<&String>) -> Self {
        match text.map(String::as_str) {
            Some("account") => Self::Account,
            Some("model") => Self::Model,
            Some("person") => Self::Person,
            Some("client") => Self::Client,
            Some("status") => Self::Status,
            _ => Self::Provider,
        }
    }

    fn column(self) -> &'static str {
        match self {
            Self::Provider => "provider",
            Self::Account => "account",
            Self::Model => "model",
            Self::Person => "person",
            Self::Client => "client",
            Self::Status => "status",
        }
    }
}

const AGGREGATE: &str = "
    COUNT(*),
    SUM(status = 'error'),
    SUM(status = 'in_flight' AND started_at >= ?),
    SUM(input_tokens), SUM(output_tokens),
    SUM(cache_read_tokens), SUM(cache_write_tokens), SUM(reasoning_tokens),
    SUM(cost_micros), SUM(cost_micros IS NOT NULL)
";

fn read_totals(row: &rusqlite::Row<'_>, from: usize) -> rusqlite::Result<Totals> {
    Ok(Totals {
        requests: row.get(from)?,
        errors: row.get::<_, Option<i64>>(from + 1)?.unwrap_or(0),
        in_flight: row.get::<_, Option<i64>>(from + 2)?.unwrap_or(0),
        usage: Usage {
            input_tokens: row.get(from + 3)?,
            output_tokens: row.get(from + 4)?,
            cache_read_tokens: row.get(from + 5)?,
            cache_write_tokens: row.get(from + 6)?,
            reasoning_tokens: row.get(from + 7)?,
        },
        cost_micros: row.get(from + 8)?,
        priced_requests: row.get::<_, Option<i64>>(from + 9)?.unwrap_or(0),
        median_ms: None,
        slowest_tenth_ms: None,
    })
}

impl Store {
    pub fn totals(&self, filter: &Filter) -> Result<Totals, StoreError> {
        let (where_clause, narrowing) = filter.sql();
        let mut counted = vec![Sql::Integer(now() - ABANDONED_AFTER_MS)];
        counted.extend(narrowing.iter().cloned());

        let connection = self.connection.lock().expect("store mutex");
        let mut totals = connection.query_row(
            &format!("SELECT {AGGREGATE} FROM requests WHERE {where_clause}"),
            params_from_iter(counted.iter()),
            |row| read_totals(row, 0),
        )?;

        let mut statement = connection.prepare(&format!(
            "SELECT duration_ms FROM requests
             WHERE {where_clause} AND status = 'ok' AND duration_ms IS NOT NULL
             ORDER BY duration_ms"
        ))?;
        let durations = statement
            .query_map(params_from_iter(narrowing.iter()), |row| {
                row.get::<_, i64>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        totals.median_ms = at_quantile(&durations, 50);
        totals.slowest_tenth_ms = at_quantile(&durations, 90);
        Ok(totals)
    }

    pub fn breakdown(&self, filter: &Filter, by: By) -> Result<Vec<Slice>, StoreError> {
        let (where_clause, mut binds) = filter.sql();
        binds.insert(0, Sql::Integer(now() - ABANDONED_AFTER_MS));
        let column = by.column();

        let connection = self.connection.lock().expect("store mutex");
        let mut statement = connection.prepare(&format!(
            "SELECT COALESCE({column}, ''), {AGGREGATE} FROM requests
             WHERE {where_clause} GROUP BY 1 ORDER BY 2 DESC"
        ))?;
        let slices = statement
            .query_map(params_from_iter(binds.iter()), |row| {
                Ok(Slice {
                    key: row.get(0)?,
                    totals: read_totals(row, 1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(slices)
    }

    pub fn series(
        &self,
        filter: &Filter,
        by: By,
        bucket_ms: i64,
    ) -> Result<Vec<(i64, Vec<Slice>)>, StoreError> {
        let (where_clause, mut binds) = filter.sql();
        binds.insert(0, Sql::Integer(now() - ABANDONED_AFTER_MS));
        let column = by.column();
        let bucket = bucket_ms.max(1);

        let connection = self.connection.lock().expect("store mutex");
        let mut statement = connection.prepare(&format!(
            "SELECT (started_at / {bucket}) * {bucket}, COALESCE({column}, ''), {AGGREGATE}
             FROM requests WHERE {where_clause} GROUP BY 1, 2 ORDER BY 1"
        ))?;
        let rows = statement
            .query_map(params_from_iter(binds.iter()), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    Slice {
                        key: row.get(1)?,
                        totals: read_totals(row, 2)?,
                    },
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut buckets: Vec<(i64, Vec<Slice>)> = Vec::new();
        for (at, slice) in rows {
            match buckets.last_mut() {
                Some((last, slices)) if *last == at => slices.push(slice),
                _ => buckets.push((at, vec![slice])),
            }
        }
        Ok(buckets)
    }

    pub fn find_requests(
        &self,
        filter: &Filter,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<RequestRow>, StoreError> {
        let (mut where_clause, mut binds) = filter.sql();
        if let Some(before) = before {
            where_clause.push_str(" AND started_at < ?");
            binds.push(Sql::Integer(before));
        }
        binds.push(Sql::Integer(limit as i64));

        let connection = self.connection.lock().expect("store mutex");
        let mut statement = connection.prepare(&format!(
            "SELECT {} FROM requests WHERE {where_clause} ORDER BY started_at DESC LIMIT ?",
            crate::store::records::REQUEST_COLUMNS
        ))?;
        let rows = statement
            .query_map(
                params_from_iter(binds.iter()),
                crate::store::records::read_request,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows.into_iter().map(settled).collect())
    }

    pub fn request(&self, id: &str) -> Result<Option<RequestRow>, StoreError> {
        use rusqlite::OptionalExtension as _;
        let connection = self.connection.lock().expect("store mutex");
        let row = connection
            .query_row(
                &format!(
                    "SELECT {} FROM requests WHERE id = ?1",
                    crate::store::records::REQUEST_COLUMNS
                ),
                [id],
                crate::store::records::read_request,
            )
            .optional()?;
        Ok(row.map(settled))
    }
}

fn settled(mut row: RequestRow) -> RequestRow {
    if row.status == "in_flight" && row.started_at < now() - ABANDONED_AFTER_MS {
        row.status = "abandoned".into();
    }
    row
}

fn at_quantile(sorted: &[i64], percent: usize) -> Option<i64> {
    if sorted.is_empty() {
        return None;
    }
    let index = (sorted.len() * percent / 100).min(sorted.len() - 1);
    Some(sorted[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::in_memory(&[3u8; 32]).unwrap()
    }

    fn row(id: &str, provider: &str, status: &str, duration: i64, cost: Option<i64>) -> RequestRow {
        RequestRow {
            id: id.to_owned(),
            started_at: now(),
            person: Some("jmo".into()),
            client: Some("Claude Code".into()),
            conversation: None,
            provider: provider.to_owned(),
            account: Some(format!("{provider}-1")),
            model: "claude-sonnet-5".into(),
            ingress: "messages".into(),
            egress: "messages".into(),
            translated: false,
            status: status.to_owned(),
            error_kind: None,
            error_message: None,
            ttft_ms: None,
            duration_ms: Some(duration),
            usage: Usage {
                input_tokens: Some(100),
                output_tokens: Some(20),
                cache_read_tokens: Some(300),
                cache_write_tokens: None,
                reasoning_tokens: None,
            },
            cost_micros: cost,
            input_digest: None,
            output_digest: None,
            byte_identical: Some(true),
            evidence: None,
            upstream_request_id: None,
        }
    }

    #[test]
    fn a_failed_request_does_not_flatter_the_latency() {
        let store = store();
        store
            .record_request(&row("a", "anthropic", "ok", 4000, None))
            .unwrap();
        store
            .record_request(&row("b", "anthropic", "ok", 6000, None))
            .unwrap();
        store
            .record_request(&row("c", "anthropic", "error", 3, None))
            .unwrap();

        let totals = store.totals(&Filter::default()).unwrap();
        assert_eq!(totals.requests, 3);
        assert_eq!(totals.errors, 1);
        assert_eq!(
            totals.median_ms,
            Some(6000),
            "an instant 401 is not the gateway being fast"
        );
    }

    #[test]
    fn a_model_with_no_price_is_counted_but_not_costed() {
        let store = store();
        store
            .record_request(&row("a", "anthropic", "ok", 10, Some(500)))
            .unwrap();
        store
            .record_request(&row("b", "anthropic", "ok", 10, None))
            .unwrap();

        let totals = store.totals(&Filter::default()).unwrap();
        assert_eq!(totals.requests, 2);
        assert_eq!(totals.cost_micros, Some(500));
        assert_eq!(
            totals.priced_requests, 1,
            "the screen has to be able to say the total is only part of the story"
        );
    }

    #[test]
    fn a_breakdown_splits_by_the_column_asked_for() {
        let store = store();
        store
            .record_request(&row("a", "anthropic", "ok", 10, None))
            .unwrap();
        store
            .record_request(&row("b", "anthropic", "ok", 10, None))
            .unwrap();
        store
            .record_request(&row("c", "openai", "error", 10, None))
            .unwrap();

        let slices = store.breakdown(&Filter::default(), By::Provider).unwrap();
        assert_eq!(slices.len(), 2);
        assert_eq!(slices[0].key, "anthropic");
        assert_eq!(slices[0].totals.requests, 2);
        assert_eq!(slices[1].totals.errors, 1);
    }

    #[test]
    fn a_filter_narrows_both_the_totals_and_the_list() {
        let store = store();
        store
            .record_request(&row("a", "anthropic", "ok", 10, None))
            .unwrap();
        store
            .record_request(&row("b", "openai", "ok", 10, None))
            .unwrap();

        let filter = Filter {
            provider: Some("openai".into()),
            ..Filter::default()
        };
        assert_eq!(store.totals(&filter).unwrap().requests, 1);
        let found = store.find_requests(&filter, None, 10).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "b");
    }

    #[test]
    fn search_reaches_the_fields_someone_would_paste_in() {
        let store = store();
        let mut tagged = row("req_abc123", "anthropic", "ok", 10, None);
        tagged.upstream_request_id = Some("req_011CSHoEeq".into());
        store.record_request(&tagged).unwrap();
        store
            .record_request(&row("other", "anthropic", "ok", 10, None))
            .unwrap();

        let hit = |text: &str| {
            store
                .find_requests(
                    &Filter {
                        search: Some(text.into()),
                        ..Filter::default()
                    },
                    None,
                    10,
                )
                .unwrap()
                .len()
        };
        assert_eq!(hit("011CSHoEeq"), 1);
        assert_eq!(hit("abc123"), 1);
        assert_eq!(hit("sonnet"), 2);
    }

    #[test]
    fn a_request_that_never_ended_stops_claiming_to_be_running() {
        let store = store();
        let mut live = row("live", "anthropic", "in_flight", 0, None);
        live.duration_ms = None;
        store.record_request(&live).unwrap();

        let mut stuck = row("stuck", "anthropic", "in_flight", 0, None);
        stuck.id = "stuck".into();
        stuck.started_at = now() - ABANDONED_AFTER_MS - 1;
        stuck.duration_ms = None;
        store.record_request(&stuck).unwrap();

        assert_eq!(store.totals(&Filter::default()).unwrap().in_flight, 1);
        assert_eq!(store.request("stuck").unwrap().unwrap().status, "abandoned");
        assert_eq!(store.request("live").unwrap().unwrap().status, "in_flight");
    }

    #[test]
    fn a_series_puts_everything_from_one_slot_in_one_bucket() {
        let store = store();
        store
            .record_request(&row("a", "anthropic", "ok", 10, None))
            .unwrap();
        store
            .record_request(&row("b", "openai", "ok", 10, None))
            .unwrap();

        let buckets = store
            .series(&Filter::default(), By::Provider, 3_600_000)
            .unwrap();
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets[0].1.len(), 2);
    }
}
