use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use axum::http::HeaderMap;

pub const HEADER: &str = "x-codex-turn-state";
const REMEMBERED: usize = 512;

#[derive(Clone, Default)]
pub struct Turns {
    seen: Arc<Mutex<VecDeque<(String, String)>>>,
}

impl Turns {
    pub fn minted(&self, headers: &HeaderMap, account: &str) {
        let Some(state) = state_in(headers) else {
            return;
        };
        let mut seen = self.seen.lock().expect("turns mutex");
        if seen.iter().any(|(held, _)| held == &state) {
            return;
        }
        if seen.len() >= REMEMBERED {
            seen.pop_front();
        }
        seen.push_back((state, account.to_owned()));
    }

    pub fn account_for(&self, headers: &HeaderMap) -> Option<String> {
        let state = state_in(headers)?;
        let seen = self.seen.lock().expect("turns mutex");
        seen.iter()
            .find(|(held, _)| held == &state)
            .map(|(_, account)| account.clone())
    }
}

fn state_in(headers: &HeaderMap) -> Option<String> {
    headers
        .get(HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|state| !state.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn carrying(state: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(HEADER, HeaderValue::from_str(state).unwrap());
        headers
    }

    #[test]
    fn a_turn_goes_back_to_the_account_that_opened_it() {
        let turns = Turns::default();
        turns.minted(&carrying("ts_abc"), "personal");
        assert_eq!(
            turns.account_for(&carrying("ts_abc")).as_deref(),
            Some("personal")
        );
    }

    #[test]
    fn a_turn_state_is_written_once_and_not_moved_afterwards() {
        let turns = Turns::default();
        turns.minted(&carrying("ts_abc"), "personal");
        turns.minted(&carrying("ts_abc"), "work");
        assert_eq!(
            turns.account_for(&carrying("ts_abc")).as_deref(),
            Some("personal")
        );
    }

    #[test]
    fn a_request_carrying_nothing_pins_to_nothing() {
        let turns = Turns::default();
        turns.minted(&carrying("ts_abc"), "personal");
        assert_eq!(turns.account_for(&HeaderMap::new()), None);
        assert_eq!(turns.account_for(&carrying("ts_other")), None);
    }

    #[test]
    fn old_turns_make_room_for_new_ones() {
        let turns = Turns::default();
        let sent = REMEMBERED + 10;
        for index in 0..sent {
            turns.minted(&carrying(&format!("ts_{index}")), "personal");
        }
        assert_eq!(
            turns.account_for(&carrying("ts_0")),
            None,
            "a turn this old ended long ago"
        );
        assert!(
            turns
                .account_for(&carrying(&format!("ts_{}", sent - 1)))
                .is_some(),
            "the turn that just opened has to still be pinned"
        );
    }
}
