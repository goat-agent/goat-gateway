use serde::Serialize;

pub const AS_OF: &str = "2026-08";

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Price {
    pub model: &'static str,
    pub input_per_mtok_micros: i64,
    pub output_per_mtok_micros: i64,
    pub cache_read_per_mtok_micros: i64,
    pub cache_write_per_mtok_micros: i64,
}

const TABLE: &[Price] = &[
    Price {
        model: "claude-opus-5",
        input_per_mtok_micros: 15_000_000,
        output_per_mtok_micros: 75_000_000,
        cache_read_per_mtok_micros: 1_500_000,
        cache_write_per_mtok_micros: 18_750_000,
    },
    Price {
        model: "claude-sonnet-5",
        input_per_mtok_micros: 3_000_000,
        output_per_mtok_micros: 15_000_000,
        cache_read_per_mtok_micros: 300_000,
        cache_write_per_mtok_micros: 3_750_000,
    },
    Price {
        model: "claude-haiku-4-5",
        input_per_mtok_micros: 1_000_000,
        output_per_mtok_micros: 5_000_000,
        cache_read_per_mtok_micros: 100_000,
        cache_write_per_mtok_micros: 1_250_000,
    },
];

pub fn price(model: &str) -> Option<Price> {
    TABLE.iter().copied().find(|row| row.model == model)
}

pub fn table() -> &'static [Price] {
    TABLE
}

pub fn cost_micros(model: &str, usage: &crate::store::Usage) -> Option<i64> {
    let price = price(model)?;
    let per = |tokens: Option<i64>, rate: i64| tokens.unwrap_or(0) * rate / 1_000_000;
    Some(
        per(usage.input_tokens, price.input_per_mtok_micros)
            + per(usage.output_tokens, price.output_per_mtok_micros)
            + per(usage.cache_read_tokens, price.cache_read_per_mtok_micros)
            + per(usage.cache_write_tokens, price.cache_write_per_mtok_micros),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Usage;

    #[test]
    fn an_unpriced_model_costs_unknown_not_zero() {
        assert_eq!(cost_micros("claude-sonnet-6", &Usage::default()), None);
    }

    #[test]
    fn cache_reads_are_priced_separately_from_fresh_input() {
        let usage = Usage {
            input_tokens: Some(1_000_000),
            cache_read_tokens: Some(1_000_000),
            ..Usage::default()
        };
        let cost = cost_micros("claude-sonnet-5", &usage).unwrap();
        assert_eq!(cost, 3_000_000 + 300_000);
    }

    #[test]
    fn the_table_carries_the_date_it_was_true() {
        assert_eq!(AS_OF, "2026-08");
    }
}
