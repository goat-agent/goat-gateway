use axum::{
    Router,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rust_embed::Embed;

use crate::App;

#[derive(Embed)]
#[folder = "../../web/dist"]
#[allow_missing = true]
struct Assets;

pub fn router() -> Router<App> {
    Router::new().route("/", get(index)).fallback(get(asset))
}

async fn index() -> Response {
    if Assets::get("index.html").is_none() {
        return (
            StatusCode::NOT_FOUND,
            "This build has no console: it was compiled with GOAT_SKIP_WEB_BUILD set. \
             Build again without it, or run `npm --prefix web run build` first.",
        )
            .into_response();
    }
    serve("index.html")
}

async fn asset(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    if path.starts_with("api/") || path.starts_with("v1/") {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }
    if path.is_empty() || Assets::get(path).is_none() {
        return serve("index.html");
    }
    serve(path)
}

fn serve(path: &str) -> Response {
    match Assets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                file.data.into_owned(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}
