use crate::{
    limits::Snapshot,
    store::{AccountState, Store, StoreError, now},
};

#[derive(Debug, Clone)]
pub struct Chosen {
    pub name: String,
    pub credential_kind: String,
    pub secret: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("no account is registered for {provider}")]
    NoneRegistered { provider: String },
    #[error("every {provider} account is unavailable right now. {detail}")]
    AllUnavailable { provider: String, detail: String },
    #[error(
        "this conversation carries reasoning state minted by account {pinned:?}, which is {state}. \
         Sending it anywhere else would make the model restart its reasoning."
    )]
    PinUnavailable { pinned: String, state: String },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Want<'a> {
    pub provider: &'a str,
    pub scope: Option<&'a str>,
    pub pinned: Option<&'a str>,
    pub prefer: Option<&'a str>,
}

impl<'a> Want<'a> {
    pub fn from(provider: &'a str) -> Self {
        Self {
            provider,
            ..Self::default()
        }
    }
}

pub fn pick(store: &Store, want: &Want<'_>) -> Result<Chosen, PoolError> {
    let Want {
        provider,
        scope,
        pinned,
        prefer,
    } = *want;

    store.clear_expired_cooldowns()?;
    let accounts = store.accounts()?;

    let candidates: Vec<_> = accounts
        .iter()
        .filter(|account| account.provider == provider)
        .collect();
    if candidates.is_empty() {
        return Err(PoolError::NoneRegistered {
            provider: provider.to_owned(),
        });
    }

    if let Some(pinned) = pinned {
        let account = candidates
            .iter()
            .find(|account| account.name == pinned)
            .ok_or_else(|| PoolError::PinUnavailable {
                pinned: pinned.to_owned(),
                state: "no longer registered".to_owned(),
            })?;
        if account.state != AccountState::Active {
            return Err(PoolError::PinUnavailable {
                pinned: pinned.to_owned(),
                state: describe(account.state, account.cooldown_until),
            });
        }
        return load(store, pinned);
    }

    let mut usable: Vec<_> = candidates
        .iter()
        .filter(|account| account.state == AccountState::Active)
        .collect();

    if let Some(prefer) = prefer
        && usable.iter().any(|account| account.name == prefer)
    {
        return load(store, prefer);
    }

    if usable.is_empty() {
        let detail = candidates
            .iter()
            .map(|account| {
                format!(
                    "{} is {}",
                    account.name,
                    describe(account.state, account.cooldown_until)
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        return Err(PoolError::AllUnavailable {
            provider: provider.to_owned(),
            detail,
        });
    }

    usable.sort_by(|a, b| {
        let pressure = |name: &str| {
            store
                .rate_limits(name)
                .ok()
                .flatten()
                .and_then(|snapshot: Snapshot| snapshot.pressure_for(scope))
                .unwrap_or(f64::NEG_INFINITY)
        };
        pressure(&a.name)
            .partial_cmp(&pressure(&b.name))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });

    load(store, &usable[0].name)
}

fn load(store: &Store, name: &str) -> Result<Chosen, PoolError> {
    let accounts = store.accounts()?;
    let row = accounts
        .into_iter()
        .find(|account| account.name == name)
        .ok_or_else(|| StoreError::UnknownAccount(name.to_owned()))?;
    Ok(Chosen {
        name: row.name,
        credential_kind: row.credential_kind,
        secret: store.secret(name)?,
    })
}

fn describe(state: AccountState, cooldown_until: Option<i64>) -> String {
    match state {
        AccountState::Active => "available".to_owned(),
        AccountState::Disabled => "turned off".to_owned(),
        AccountState::SignInExpired => "signed out and needs a new login".to_owned(),
        AccountState::RateLimited => match cooldown_until {
            Some(at) => {
                let seconds = ((at - now()) / 1000).max(0);
                format!("rate limited for another {seconds}s")
            }
            None => "rate limited".to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::Window;

    fn store() -> Store {
        Store::in_memory(&[1u8; 32]).unwrap()
    }

    fn snapshot(used: f64) -> Snapshot {
        Snapshot {
            windows: vec![Window {
                label: "5h".into(),
                scope: None,
                used_percent: used,
                resets_at_ms: None,
            }],
            binding: None,
        }
    }

    #[test]
    fn a_full_model_bucket_does_not_take_the_whole_account_away() {
        let store = store();
        store
            .add_account("shared", "anthropic", "api_key", b"a")
            .unwrap();
        store
            .set_rate_limits(
                "shared",
                &Snapshot {
                    windows: vec![
                        Window {
                            label: "5h".into(),
                            scope: None,
                            used_percent: 20.0,
                            resets_at_ms: None,
                        },
                        Window {
                            label: "weekly".into(),
                            scope: Some("fable".into()),
                            used_percent: 100.0,
                            resets_at_ms: None,
                        },
                    ],
                    binding: None,
                },
            )
            .unwrap();

        let snapshot = store.rate_limits("shared").unwrap().unwrap();
        assert_eq!(snapshot.pressure_for(Some("fable")), Some(100.0));
        assert_eq!(snapshot.pressure_for(Some("sonnet")), Some(20.0));
        assert_eq!(snapshot.pressure_for(None), Some(20.0));
    }

    #[test]
    fn an_empty_pool_says_so_plainly() {
        let error = pick(&store(), &Want::from("anthropic")).unwrap_err();
        assert!(matches!(error, PoolError::NoneRegistered { .. }));
    }

    #[test]
    fn the_least_pressured_account_wins() {
        let store = store();
        for (name, used) in [("busy", 91.0), ("idle", 12.0), ("middling", 55.0)] {
            store
                .add_account(name, "anthropic", "api_key", name.as_bytes())
                .unwrap();
            store.set_rate_limits(name, &snapshot(used)).unwrap();
        }
        assert_eq!(pick(&store, &Want::from("anthropic")).unwrap().name, "idle");
    }

    #[test]
    fn an_account_with_no_reported_limit_is_tried_first() {
        let store = store();
        store
            .add_account("known", "anthropic", "api_key", b"a")
            .unwrap();
        store.set_rate_limits("known", &snapshot(10.0)).unwrap();
        store
            .add_account("unknown", "anthropic", "api_key", b"b")
            .unwrap();

        assert_eq!(
            pick(&store, &Want::from("anthropic")).unwrap().name,
            "unknown"
        );
    }

    #[test]
    fn cooling_accounts_are_skipped_and_recovered_accounts_return() {
        let store = store();
        store
            .add_account("a", "anthropic", "api_key", b"a")
            .unwrap();
        store
            .add_account("b", "anthropic", "api_key", b"b")
            .unwrap();
        store
            .set_state("a", AccountState::RateLimited, Some(now() + 60_000))
            .unwrap();
        assert_eq!(pick(&store, &Want::from("anthropic")).unwrap().name, "b");

        store
            .set_state("a", AccountState::RateLimited, Some(now() - 1))
            .unwrap();
        store.remove_account("b").unwrap();
        assert_eq!(pick(&store, &Want::from("anthropic")).unwrap().name, "a");
    }

    #[test]
    fn when_nothing_is_usable_the_error_names_every_reason() {
        let store = store();
        store
            .add_account("a", "anthropic", "api_key", b"a")
            .unwrap();
        store
            .add_account("b", "anthropic", "api_key", b"b")
            .unwrap();
        store
            .set_state("a", AccountState::RateLimited, Some(now() + 90_000))
            .unwrap();
        store
            .set_state("b", AccountState::SignInExpired, None)
            .unwrap();

        let message = pick(&store, &Want::from("anthropic"))
            .unwrap_err()
            .to_string();
        assert!(
            message.contains("a is rate limited for another"),
            "{message}"
        );
        assert!(message.contains("b is signed out"), "{message}");
    }

    #[test]
    fn a_pinned_account_is_used_even_when_it_is_not_the_calmest() {
        let store = store();
        store
            .add_account("busy", "anthropic", "api_key", b"a")
            .unwrap();
        store.set_rate_limits("busy", &snapshot(99.0)).unwrap();
        store
            .add_account("idle", "anthropic", "api_key", b"b")
            .unwrap();
        store.set_rate_limits("idle", &snapshot(1.0)).unwrap();

        assert_eq!(
            pick(
                &store,
                &Want {
                    pinned: Some("busy"),
                    ..Want::from("anthropic")
                }
            )
            .unwrap()
            .name,
            "busy"
        );
    }

    #[test]
    fn a_conversation_goes_back_to_the_account_holding_its_cache() {
        let store = store();
        store
            .add_account("warm", "anthropic", "api_key", b"a")
            .unwrap();
        store.set_rate_limits("warm", &snapshot(60.0)).unwrap();
        store
            .add_account("cold", "anthropic", "api_key", b"b")
            .unwrap();
        store.set_rate_limits("cold", &snapshot(1.0)).unwrap();

        let want = Want {
            prefer: Some("warm"),
            ..Want::from("anthropic")
        };
        assert_eq!(pick(&store, &want).unwrap().name, "warm");
    }

    #[test]
    fn a_preference_gives_way_when_that_account_cannot_serve() {
        let store = store();
        store
            .add_account("warm", "anthropic", "api_key", b"a")
            .unwrap();
        store
            .add_account("cold", "anthropic", "api_key", b"b")
            .unwrap();
        store
            .set_state("warm", AccountState::RateLimited, Some(now() + 60_000))
            .unwrap();

        let want = Want {
            prefer: Some("warm"),
            ..Want::from("anthropic")
        };
        assert_eq!(
            pick(&store, &want).unwrap().name,
            "cold",
            "a warm cache is worth less than getting an answer at all"
        );
    }

    #[test]
    fn a_pinned_account_that_is_down_fails_rather_than_silently_moving() {
        let store = store();
        store
            .add_account("busy", "anthropic", "api_key", b"a")
            .unwrap();
        store
            .add_account("idle", "anthropic", "api_key", b"b")
            .unwrap();
        store
            .set_state("busy", AccountState::RateLimited, Some(now() + 60_000))
            .unwrap();

        let error = pick(
            &store,
            &Want {
                pinned: Some("busy"),
                ..Want::from("anthropic")
            },
        )
        .unwrap_err();
        let message = error.to_string();
        assert!(matches!(error, PoolError::PinUnavailable { .. }));
        assert!(message.contains("restart its reasoning"), "{message}");
    }

    #[test]
    fn providers_do_not_borrow_each_others_accounts() {
        let store = store();
        store
            .add_account("anthropic-one", "anthropic", "api_key", b"a")
            .unwrap();
        assert!(matches!(
            pick(&store, &Want::from("openai")).unwrap_err(),
            PoolError::NoneRegistered { .. }
        ));
    }

    #[test]
    fn the_chosen_account_carries_its_decrypted_secret() {
        let store = store();
        store
            .add_account("a", "anthropic", "api_key", b"sk-live")
            .unwrap();
        assert_eq!(
            pick(&store, &Want::from("anthropic")).unwrap().secret,
            b"sk-live"
        );
    }
}
