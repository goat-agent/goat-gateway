use crate::{provider::Price, store::Usage};

pub const AS_OF: &str = "2026-08";

pub fn cost_micros(price: Price, usage: &Usage) -> i64 {
    let per_mtok = |tokens: Option<i64>, rate: i64| tokens.unwrap_or(0) * rate / 1_000_000;
    per_mtok(usage.input_tokens, price.input)
        + per_mtok(usage.output_tokens, price.output)
        + per_mtok(usage.cache_read_tokens, price.cache_read)
        + per_mtok(usage.cache_write_tokens, price.cache_write)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::Catalog;

    fn sonnet() -> Price {
        Catalog::builtin()
            .model("anthropic", "claude-sonnet-5")
            .unwrap()
            .price
            .unwrap()
    }

    #[test]
    fn cache_reads_are_charged_at_their_own_rate() {
        let usage = Usage {
            input_tokens: Some(1_000_000),
            output_tokens: Some(1_000_000),
            cache_read_tokens: Some(1_000_000),
            cache_write_tokens: None,
            reasoning_tokens: None,
        };
        assert_eq!(
            cost_micros(sonnet(), &usage),
            3_000_000 + 15_000_000 + 300_000
        );
    }

    #[test]
    fn a_token_count_we_never_saw_costs_nothing_rather_than_guessing() {
        let usage = Usage {
            input_tokens: Some(1_000_000),
            output_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            reasoning_tokens: None,
        };
        assert_eq!(cost_micros(sonnet(), &usage), 3_000_000);
    }
}
