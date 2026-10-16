use axum::{
    Json,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::{
    App,
    store::{Caller, Viewer},
};

pub const COOKIE: &str = "goat_session";
const ADMIN_KEY_COOKIE: &str = "goat_admin";

pub async fn gateway(State(app): State<App>, mut request: Request, next: Next) -> Response {
    let presented = request
        .headers()
        .get(crate::upstream::GATEWAY_KEY)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_owned())
        .or_else(|| bearer(request.headers()));
    let Some(secret) = presented else {
        return unauthorized(request.uri().path(), "no API key was sent");
    };

    match app.store().caller_for(&secret) {
        Ok(Some(caller)) => {
            request.extensions_mut().insert(caller);
            next.run(request).await
        }
        Ok(None) => unauthorized(
            request.uri().path(),
            "this API key is not valid here. Issue one on the gateway's Keys page",
        ),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

pub async fn console(State(app): State<App>, mut request: Request, next: Next) -> Response {
    match viewer_of(&app, request.headers()) {
        Some(viewer) => {
            request.extensions_mut().insert(viewer);
            next.run(request).await
        }
        None => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "sign in to use the console" })),
        )
            .into_response(),
    }
}

pub async fn admins_only(request: Request, next: Next) -> Response {
    let admin = request
        .extensions()
        .get::<Viewer>()
        .is_some_and(Viewer::is_admin);
    if admin {
        next.run(request).await
    } else {
        (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "only an admin can do this. Ask one, or sign in with the admin key" })),
        )
            .into_response()
    }
}

fn viewer_of(app: &App, headers: &HeaderMap) -> Option<Viewer> {
    if let Some(token) = cookie(headers, COOKIE)
        && let Ok(Some(viewer)) = app.store().viewer_for(&token)
    {
        return Some(viewer);
    }
    [cookie(headers, ADMIN_KEY_COOKIE), bearer(headers)]
        .into_iter()
        .flatten()
        .any(|presented| app.store().admin_key_matches(&presented).unwrap_or(false))
        .then_some(Viewer::Root)
}

pub fn session_token(headers: &HeaderMap) -> Option<String> {
    cookie(headers, COOKIE)
}

pub fn caller_of(request: &Request) -> Option<&Caller> {
    request.extensions().get::<Caller>()
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers.get("authorization").and_then(|v| v.to_str().ok())
        && let Some(rest) = value.strip_prefix("Bearer ")
    {
        return Some(rest.trim().to_owned());
    }
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().to_owned())
}

fn cookie(headers: &HeaderMap, wanted: &str) -> Option<String> {
    let jar = headers.get("cookie")?.to_str().ok()?;
    jar.split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == wanted)
        .map(|(_, value)| value.to_owned())
}

fn unauthorized(path: &str, message: &str) -> Response {
    let body = if path.ends_with("/responses") || path.ends_with("/chat/completions") {
        json!({
            "error": {
                "type": "invalid_request_error",
                "message": message,
                "code": "invalid_api_key",
                "param": null,
            }
        })
    } else {
        json!({
            "type": "error",
            "error": { "type": "authentication_error", "message": message },
        })
    };
    (StatusCode::UNAUTHORIZED, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderName, HeaderValue};

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(
                HeaderName::from_bytes(name.as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        map
    }

    #[test]
    fn both_header_styles_are_accepted() {
        assert_eq!(
            bearer(&headers(&[("authorization", "Bearer gwk_abc")])).as_deref(),
            Some("gwk_abc")
        );
        assert_eq!(
            bearer(&headers(&[("x-api-key", "gwk_abc")])).as_deref(),
            Some("gwk_abc")
        );
        assert_eq!(bearer(&headers(&[])), None);
    }

    #[test]
    fn the_cookie_is_found_among_others() {
        assert_eq!(
            cookie(
                &headers(&[("cookie", "theme=dark; goat_session=gws_abc; x=1")]),
                COOKIE
            )
            .as_deref(),
            Some("gws_abc")
        );
        assert_eq!(cookie(&headers(&[("cookie", "theme=dark")]), COOKIE), None);
    }

    #[tokio::test]
    async fn every_openai_door_is_refused_in_the_openai_shape() {
        for path in [
            "/v1/responses",
            "/responses",
            "/v1/chat/completions",
            "/chat/completions",
        ] {
            let body = axum::body::to_bytes(unauthorized(path, "no").into_body(), usize::MAX)
                .await
                .unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["error"]["code"], "invalid_api_key", "{path}: {body}");
            assert!(body.get("type").is_none(), "{path}: {body}");
        }
    }

    #[test]
    fn the_refusal_speaks_the_protocol_of_the_path() {
        let responses = unauthorized("/v1/responses", "no");
        let messages = unauthorized("/v1/messages", "no");
        assert_eq!(responses.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(messages.status(), StatusCode::UNAUTHORIZED);
    }
}
