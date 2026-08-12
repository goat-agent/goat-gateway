use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{App, limits, provider::Limits, store::now};

pub const NOT_MORE_OFTEN_THAN_MS: i64 = 60_000;

#[derive(Clone, Default)]
pub struct Asked {
    last: Arc<Mutex<HashMap<String, i64>>>,
}

impl Asked {
    fn due(&self, account: &str, at: i64) -> bool {
        let mut last = self.last.lock().expect("quota mutex");
        match last.get(account) {
            Some(when) if at - when < NOT_MORE_OFTEN_THAN_MS => false,
            _ => {
                last.insert(account.to_owned(), at);
                true
            }
        }
    }
}

pub fn refresh_soon(app: &App, account: &str, provider: &str) {
    let Some(declared) = app.catalog().get(provider) else {
        return;
    };
    let Limits::Endpoint(asked) = declared.limits.clone() else {
        return;
    };
    if !app.inner.asked.due(account, now()) {
        return;
    }

    let app = app.clone();
    let account = account.to_owned();
    let provider = provider.to_owned();

    tokio::spawn(async move {
        if let Err(error) = ask(&app, &account, &provider, &asked).await {
            tracing::debug!(%account, %error, "could not read what is left");
        }
    });
}

async fn ask(
    app: &App,
    account: &str,
    provider: &str,
    asked: &crate::provider::Asked,
) -> Result<(), String> {
    let row = app
        .store()
        .accounts()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|row| row.name == account)
        .ok_or_else(|| format!("{account} is gone"))?;

    let secret = app
        .store()
        .secret(account)
        .map_err(|error| error.to_string())?;
    let prepared = crate::oauth::prepare(
        app.store(),
        &app.oauth_client(),
        account,
        provider,
        &row.credential_kind,
        &secret,
    )
    .await
    .map_err(|error| error.to_string())?;

    let auth = match prepared.auth {
        crate::upstream::Auth::ApiKey(secret) => {
            let key = app
                .catalog()
                .get(provider)
                .map_or(crate::provider::Key::Bearer, |declared| declared.key);
            crate::upstream::Auth::presented(key, secret)
        }
        carried => carried,
    };
    let (headers, _) = crate::upstream::forward_headers(&Default::default(), &auth);

    let answered = app
        .inner
        .client
        .get(&asked.url)
        .headers(headers)
        .send()
        .await
        .map_err(|error| error.to_string())?;

    if !answered.status().is_success() {
        return Err(format!("{} answered {}", asked.url, answered.status()));
    }

    let body: serde_json::Value = answered.json().await.map_err(|error| error.to_string())?;
    let snapshot = limits::from_answer(&body, &asked.windows, now());

    app.store()
        .set_rate_limits(account, &snapshot)
        .map_err(|error| error.to_string())?;
    app.announcer()
        .say(crate::events::Happening::LimitsObserved {
            account: account.to_owned(),
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::WindowAt;
    use serde_json::json;

    fn at(label: &str, used: &str, resets: Option<&str>) -> WindowAt {
        WindowAt {
            label: label.into(),
            scope: None,
            used_percent: used.into(),
            resets_at: resets.map(str::to_owned),
            resets_in_seconds: None,
        }
    }

    #[test]
    fn the_numbers_come_from_where_the_declaration_says_they_are() {
        let body = json!({
            "data": {
                "weekly": { "used": 0.62, "reset_at": 1_800_000_000 },
                "five_hour": { "used": 91.0 },
            },
        });
        let asked = [
            at("weekly", "/data/weekly/used", Some("/data/weekly/reset_at")),
            at("5h", "/data/five_hour/used", None),
        ];

        let snapshot = limits::from_answer(&body, &asked, 1_700_000_000_000);
        assert_eq!(snapshot.windows.len(), 2);
        assert_eq!(snapshot.windows[0].used_percent, 62.0);
        assert_eq!(snapshot.windows[0].resets_at_ms, Some(1_800_000_000_000));
        assert_eq!(snapshot.windows[1].used_percent, 91.0);
        assert!(snapshot.said.is_none());
    }

    #[test]
    fn an_answer_we_were_not_taught_to_read_is_kept_so_it_can_be() {
        let body = json!({ "quota": { "remaining": 17 } });
        let snapshot = limits::from_answer(&body, &[], 0);

        assert!(snapshot.windows.is_empty());
        assert_eq!(
            snapshot.said.as_deref(),
            Some(r#"{"quota":{"remaining":17}}"#),
            "the shape has to be visible for anyone to declare pointers into it"
        );
    }

    #[test]
    fn a_pointer_that_does_not_resolve_is_left_out_rather_than_guessed() {
        let body = json!({ "data": {} });
        let snapshot = limits::from_answer(&body, &[at("weekly", "/data/missing", None)], 0);
        assert!(snapshot.windows.is_empty());
    }

    #[test]
    fn seconds_remaining_are_turned_into_a_moment() {
        let body = json!({ "left": 3600, "used": 50 });
        let asked = [WindowAt {
            label: "5h".into(),
            scope: None,
            used_percent: "/used".into(),
            resets_at: None,
            resets_in_seconds: Some("/left".into()),
        }];

        let snapshot = limits::from_answer(&body, &asked, 1_000_000);
        assert_eq!(
            snapshot.windows[0].resets_at_ms,
            Some(1_000_000 + 3_600_000)
        );
    }

    #[test]
    fn one_account_is_not_asked_twice_in_a_minute() {
        let asked = Asked::default();
        assert!(asked.due("personal", 1_000_000));
        assert!(!asked.due("personal", 1_000_000 + NOT_MORE_OFTEN_THAN_MS - 1));
        assert!(asked.due("personal", 1_000_000 + NOT_MORE_OFTEN_THAN_MS));
        assert!(asked.due("work", 1_000_000));
    }
}
