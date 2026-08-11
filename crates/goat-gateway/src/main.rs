use std::net::{IpAddr, SocketAddr};

use goat_gateway::{
    App, envelope_key_from, load_catalog,
    store::{ADMIN_PREFIX, Store, data_dir, load_or_create_master_key, mint},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "goat_gateway=info".into()),
        )
        .init();

    let dir = data_dir();
    let master_key = load_or_create_master_key(&dir)?;
    let store = Store::open(&dir.join("data.db"), &master_key)?;

    if std::env::args().nth(1).as_deref() == Some("rotate") {
        let key = mint(ADMIN_PREFIX);
        store.set_admin_key(&key)?;
        println!("New admin key. Open sessions are now signed out.\n\n    {key}\n");
        return Ok(());
    }

    let host: IpAddr = std::env::var("GOAT_HOST")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(IpAddr::from([127, 0, 0, 1]));
    let port: u16 = std::env::var("GOAT_PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8787);
    let addr = SocketAddr::new(host, port);

    if let Ok(key) = std::env::var("GOAT_ADMIN_KEY")
        && !key.is_empty()
    {
        store.set_admin_key(&key)?;
    } else if !store.has_admin_key()? {
        if !addr.ip().is_loopback() {
            return Err(format!(
                "refusing to listen on {addr} without an admin key. \
                 Set GOAT_ADMIN_KEY, or start on localhost once to have one generated."
            )
            .into());
        }
        let key = mint(ADMIN_PREFIX);
        store.set_admin_key(&key)?;
        println!("\nAdmin key — this is the only time it is shown:\n\n    {key}\n");
    }

    if let Some(secret) = std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|key| !key.is_empty())
        && store.accounts()?.is_empty()
    {
        store.add_account("Anthropic", "anthropic", "api_key", secret.as_bytes())?;
        tracing::info!("registered the account from ANTHROPIC_API_KEY");
    }

    let app = App::new(store, envelope_key_from(&master_key), load_catalog(&dir)?);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!(%addr, data = %dir.display(), "listening");
    axum::serve(listener, app.router())
        .with_graceful_shutdown(shutdown())
        .await?;

    Ok(())
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}
