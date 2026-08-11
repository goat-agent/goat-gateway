use axum::{
    Json,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::{App, store::Caller};

pub const COOKIE: &str = "goat_admin";

pub async fn gateway(State(app): State<App>, mut request: Request, next: Next) -> Response {
    let Some(secret) = bearer(request.headers()) else {
        return unauthorized(request.uri().path(), "no API key was sent");
    };

    match app.store().caller_for(&secret) {
        Ok(Some(caller)) => {
            request.extensions_mut().insert(caller);
            next.run(request).await
        }
        Ok(None) => unauthorized(
            request.uri().path(),
            "this API key is not valid here. Issue one from the gateway's Connect screen",
        ),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

pub async fn admin(State(app): State<App>, request: Request, next: Next) -> Response {
    let presented = cookie(request.headers()).or_else(|| bearer(request.headers()));

    let allowed = presented
        .as_deref()
        .map(|secret| app.store().admin_key_matches(secret).unwrap_or(false))
        .unwrap_or(false);

    if allowed {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "an admin key is required" })),
        )
            .into_response()
    }
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

fn cookie(headers: &HeaderMap) -> Option<String> {
    let jar = headers.get("cookie")?.to_str().ok()?;
    jar.split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value.to_owned())
}

fn unauthorized(path: &str, message: &str) -> Response {
    let body = if path.starts_with("/v1/responses") {
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
            cookie(&headers(&[(
                "cookie",
                "theme=dark; goat_admin=gwa_abc; x=1"
            )]))
            .as_deref(),
            Some("gwa_abc")
        );
        assert_eq!(cookie(&headers(&[("cookie", "theme=dark")])), None);
    }

    #[test]
    fn the_refusal_speaks_the_protocol_of_the_path() {
        let responses = unauthorized("/v1/responses", "no");
        let messages = unauthorized("/v1/messages", "no");
        assert_eq!(responses.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(messages.status(), StatusCode::UNAUTHORIZED);
    }
}
