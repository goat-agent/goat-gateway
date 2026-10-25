use crate::{
    limits::Snapshot,
    store::{AccountRow, AccountState, Store, StoreError, now},
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
    #[error("no account is registered for {providers}")]
    NoneRegistered { providers: String },
    #[error("every account for {providers} is unavailable right now. {detail}")]
    AllUnavailable { providers: String, detail: String },
    #[error(
        "this conversation carries reasoning state minted by account {pinned:?}, which is {state}. \
         Sending it anywhere else would make the model restart its reasoning."
    )]
    PinUnavailable { pinned: String, state: String },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Want<'a> {
    pub providers: &'a [&'a str],
    pub scope: Option<&'a str>,
    pub pinned: Option<&'a str>,
    pub prefer: Option<&'a str>,
    pub person: Option<&'a str>,
}

impl<'a> Want<'a> {
    pub fn from(providers: &'a [&'a str]) -> Self {
        Self {
            providers,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub name: String,
    pub provider: String,
    pub credential_kind: String,
}

pub fn pick(store: &Store, want: &Want<'_>) -> Result<Chosen, PoolError> {
    let first =
        candidates(store, want)?
            .into_iter()
            .next()
            .ok_or_else(|| PoolError::NoneRegistered {
                providers: want.providers.join(" or "),
            })?;
    load(store, &first.name)
}

pub fn candidates(store: &Store, want: &Want<'_>) -> Result<Vec<Candidate>, PoolError> {
    let Want {
        providers,
        scope,
        pinned,
        prefer,
        person,
    } = *want;

    store.clear_expired_cooldowns()?;
    let accounts = store.accounts()?;
    let visible = |owner: &Option<String>| match (owner, person) {
        (None, _) | (_, None) => true,
        (Some(owner), Some(person)) => owner == person,
    };

    let reachable: Vec<_> = accounts
        .iter()
        .filter(|account| providers.contains(&account.provider.as_str()) && visible(&account.owner))
        .collect();
    if reachable.is_empty() {
        return Err(PoolError::NoneRegistered {
            providers: providers.join(" or "),
        });
    }

    if let Some(pinned) = pinned {
        let account = reachable
            .iter()
            .find(|account| account.name == pinned)
            .ok_or_else(|| PoolError::PinUnavailable {
                pinned: pinned.to_owned(),
                state: "not one of the accounts this key can use".to_owned(),
            })?;
        if account.state != AccountState::Active {
            return Err(PoolError::PinUnavailable {
                pinned: pinned.to_owned(),
                state: describe(account.state, account.cooldown_until),
            });
        }
        return Ok(vec![candidate(account)]);
    }

    let mut usable: Vec<_> = reachable
        .iter()
        .filter(|account| account.state == AccountState::Active)
        .collect();

    if usable.is_empty() {
        let detail = reachable
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
            providers: providers.join(" or "),
            detail,
        });
    }

    let pressure = |name: &str| {
        store
            .rate_limits(name)
            .ok()
            .flatten()
            .and_then(|snapshot: Snapshot| snapshot.pressure_for(scope))
            .unwrap_or(f64::NEG_INFINITY)
    };
    let shared = |account: &AccountRow| account.owner.is_none() && person.is_some();
    let cold = |account: &AccountRow| prefer != Some(account.name.as_str());

    usable.sort_by(|a, b| {
        shared(a)
            .cmp(&shared(b))
            .then_with(|| cold(a).cmp(&cold(b)))
            .then_with(|| {
                pressure(&a.name)
                    .partial_cmp(&pressure(&b.name))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(usable
        .into_iter()
        .map(|account| candidate(account))
        .collect())
}

fn candidate(account: &AccountRow) -> Candidate {
    Candidate {
        name: account.name.clone(),
        provider: account.provider.clone(),
        credential_kind: account.credential_kind.clone(),
    }
}

pub fn load(store: &Store, name: &str) -> Result<Chosen, PoolError> {
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
            said: None,
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
                    said: None,
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
        let error = pick(&store(), &Want::from(&["anthropic"])).unwrap_err();
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
        assert_eq!(
            pick(&store, &Want::from(&["anthropic"])).unwrap().name,
            "idle"
        );
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
            pick(&store, &Want::from(&["anthropic"])).unwrap().name,
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
        assert_eq!(pick(&store, &Want::from(&["anthropic"])).unwrap().name, "b");

        store
            .set_state("a", AccountState::RateLimited, Some(now() - 1))
            .unwrap();
        store.remove_account("b").unwrap();
        assert_eq!(pick(&store, &Want::from(&["anthropic"])).unwrap().name, "a");
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

        let message = pick(&store, &Want::from(&["anthropic"]))
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
                    ..Want::from(&["anthropic"])
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
            ..Want::from(&["anthropic"])
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
            ..Want::from(&["anthropic"])
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
                ..Want::from(&["anthropic"])
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
            pick(&store, &Want::from(&["openai"])).unwrap_err(),
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
            pick(&store, &Want::from(&["anthropic"])).unwrap().secret,
            b"sk-live"
        );
    }

    fn two_people() -> (Store, String, String) {
        let store = store();
        let ana = store.add_user("ana").unwrap();
        let mino = store.add_user("mino").unwrap();
        for (name, owner) in [
            ("Ana Max", Some(ana.id.as_str())),
            ("Mino Max", Some(mino.id.as_str())),
            ("Team key", None),
        ] {
            store.add_account(name, "anthropic", "oauth", b"x").unwrap();
            store.set_owner(name, owner).unwrap();
        }
        (store, ana.id, mino.id)
    }

    fn names(store: &Store, want: &Want<'_>) -> Vec<String> {
        candidates(store, want)
            .unwrap()
            .into_iter()
            .map(|candidate| candidate.name)
            .collect()
    }

    #[test]
    fn someone_elses_plan_is_never_a_candidate() {
        let (store, ana, _) = two_people();
        let want = Want {
            person: Some(&ana),
            ..Want::from(&["anthropic"])
        };
        assert_eq!(names(&store, &want), ["Ana Max", "Team key"]);
    }

    #[test]
    fn your_own_account_comes_before_the_shared_one() {
        let (store, ana, _) = two_people();
        store.set_rate_limits("Ana Max", &snapshot(90.0)).unwrap();
        let want = Want {
            person: Some(&ana),
            ..Want::from(&["anthropic"])
        };
        assert_eq!(
            names(&store, &want)[0],
            "Ana Max",
            "pressure only orders accounts that are equally yours"
        );
    }

    #[test]
    fn a_pin_to_an_account_you_cannot_see_is_refused() {
        let (store, ana, _) = two_people();
        let want = Want {
            person: Some(&ana),
            pinned: Some("Mino Max"),
            ..Want::from(&["anthropic"])
        };
        assert!(matches!(
            candidates(&store, &want),
            Err(PoolError::PinUnavailable { .. })
        ));
    }

    #[test]
    fn a_plan_and_a_key_from_different_providers_serving_one_model_are_one_pool() {
        let store = store();
        let ana = store.add_user("ana").unwrap();
        store
            .add_account("Plan", "chatgpt", "oauth", b"plan")
            .unwrap();
        store.set_owner("Plan", Some(&ana.id)).unwrap();
        store
            .add_account("Key", "openai", "api_key", b"key")
            .unwrap();
        let want = Want {
            person: Some(&ana.id),
            ..Want::from(&["chatgpt", "openai"])
        };
        assert_eq!(names(&store, &want), ["Plan", "Key"]);

        store
            .set_state("Plan", AccountState::RateLimited, Some(now() + 60_000))
            .unwrap();
        assert_eq!(
            names(&store, &want),
            ["Key"],
            "a spent plan hands its model to the key rather than failing"
        );
    }
}
