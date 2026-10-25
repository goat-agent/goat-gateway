use std::net::SocketAddr;

use goat_gateway::{
    App,
    store::{RequestRow, Role, Store},
};
use serde_json::{Value, json};

const ADMIN: &str = "gwa_test-admin";

async fn serve(router: axum::Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    addr
}

struct Harness {
    addr: SocketAddr,
    store: Store,
    ana: String,
    mino: String,
}

async fn harness() -> Harness {
    let store = Store::in_memory(&[7u8; 32]).unwrap();
    store.set_admin_key(ADMIN).unwrap();
    let mino = store.add_user("mino").unwrap();
    store.set_role(&mino.id, Role::Admin).unwrap();
    let ana = store.add_user("ana").unwrap();

    store
        .add_account("Mino Max", "anthropic", "oauth", b"{\"access\":\"a\"}")
        .unwrap();
    store.set_owner("Mino Max", Some(&mino.id)).unwrap();
    store
        .add_account("Ana Max", "anthropic", "oauth", b"{\"access\":\"b\"}")
        .unwrap();
    store.set_owner("Ana Max", Some(&ana.id)).unwrap();
    store
        .add_account("Team key", "anthropic", "api_key", b"sk-team")
        .unwrap();

    let app = App::new(
        store.clone(),
        [3u8; 32],
        goat_gateway::provider::Catalog::builtin(),
    );
    let addr = serve(app.router()).await;
    Harness {
        addr,
        store,
        ana: ana.id,
        mino: mino.id,
    }
}

impl Harness {
    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    fn as_person(&self, id: &str) -> String {
        let token = self.store.open_session(id).unwrap();
        format!("goat_session={token}")
    }

    async fn get(&self, path: &str, cookie: &str) -> (u16, Value) {
        let response = reqwest::Client::new()
            .get(self.url(path))
            .header("cookie", cookie)
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        (status, response.json().await.unwrap_or(Value::Null))
    }

    async fn post(&self, path: &str, cookie: &str, body: Value) -> (u16, Value) {
        let response = reqwest::Client::new()
            .post(self.url(path))
            .header("cookie", cookie)
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        (status, response.json().await.unwrap_or(Value::Null))
    }
}

fn names(listing: &Value, field: &str) -> Vec<String> {
    let mut found: Vec<String> = listing[field]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["name"].as_str().map(str::to_owned))
        .collect();
    found.sort();
    found
}

#[tokio::test]
async fn the_cookie_an_admin_key_opens_is_not_the_admin_key() {
    let harness = harness().await;
    let response = reqwest::Client::new()
        .post(harness.url("/api/session"))
        .json(&json!({ "key": ADMIN, "name": "mino", "password": "a long password" }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);
    let cookie = response.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(cookie.contains("HttpOnly"));
    assert!(
        !cookie.contains(ADMIN),
        "a cookie that holds the admin key leaks it to every browser extension that reads cookies"
    );
}

#[tokio::test]
async fn an_invited_person_sets_a_password_and_is_signed_in_as_themselves() {
    let harness = harness().await;
    let admin = harness.as_person(&harness.mino);

    let (status, created) = harness
        .post("/api/users", &admin, json!({ "name": "jun" }))
        .await;
    assert_eq!(status, 201, "{created}");
    let invite = created["invite"].as_str().unwrap().to_owned();

    let (status, invited) = harness.get(&format!("/api/invites/{invite}"), "").await;
    assert_eq!(status, 200);
    assert_eq!(invited["name"], "jun");

    let accepted = reqwest::Client::new()
        .post(harness.url(&format!("/api/invites/{invite}")))
        .json(&json!({ "password": "a long enough password" }))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), 204);
    let cookie = accepted.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();

    let (status, me) = harness.get("/api/me", &cookie).await;
    assert_eq!(status, 200);
    assert_eq!(me["person"]["name"], "jun");
    assert_eq!(me["person"]["role"], "member");

    let signed_in = reqwest::Client::new()
        .post(harness.url("/api/session"))
        .json(&json!({ "name": "jun", "password": "a long enough password" }))
        .send()
        .await
        .unwrap();
    assert_eq!(signed_in.status(), 204);
}

#[tokio::test]
async fn a_short_password_is_refused_with_the_rule_it_broke() {
    let harness = harness().await;
    let invite = harness.store.invite(&harness.ana).unwrap();
    let (status, refused) = harness
        .post(
            &format!("/api/invites/{invite}"),
            "",
            json!({ "password": "short" }),
        )
        .await;
    assert_eq!(status, 400);
    assert!(
        refused["error"].as_str().unwrap().contains("8"),
        "{refused}"
    );
}

#[tokio::test]
async fn a_member_sees_their_own_accounts_and_the_shared_ones_and_nobody_elses() {
    let harness = harness().await;
    let ana = harness.as_person(&harness.ana);
    let (status, listing) = harness.get("/api/accounts", &ana).await;
    assert_eq!(status, 200);
    assert_eq!(names(&listing, "accounts"), ["Ana Max", "Team key"]);

    let admin = harness.as_person(&harness.mino);
    let (_, everything) = harness.get("/api/accounts", &admin).await;
    assert_eq!(
        names(&everything, "accounts"),
        ["Ana Max", "Mino Max", "Team key"]
    );
}

#[tokio::test]
async fn a_member_cannot_touch_an_account_that_is_not_theirs() {
    let harness = harness().await;
    let ana = harness.as_person(&harness.ana);
    for path in [
        "/api/accounts/Mino%20Max/state",
        "/api/accounts/Team%20key/state",
    ] {
        let (status, _) = harness
            .post(path, &ana, json!({ "state": "disabled" }))
            .await;
        assert_eq!(status, 403, "{path}");
    }
    let (status, _) = harness
        .post(
            "/api/accounts/Ana%20Max/state",
            &ana,
            json!({ "state": "disabled" }),
        )
        .await;
    assert_eq!(status, 204);
}

#[tokio::test]
async fn people_are_managed_by_admins_only() {
    let harness = harness().await;
    let ana = harness.as_person(&harness.ana);
    assert_eq!(harness.get("/api/users", &ana).await.0, 403);
    assert_eq!(
        harness
            .post("/api/users", &ana, json!({ "name": "eve" }))
            .await
            .0,
        403
    );
}

#[tokio::test]
async fn a_member_sees_only_their_own_requests() {
    let harness = harness().await;
    for (id, user) in [("req_ana", &harness.ana), ("req_mino", &harness.mino)] {
        harness
            .store
            .record_request(&RequestRow {
                id: id.to_owned(),
                started_at: goat_gateway::store::now(),
                provider: "anthropic".into(),
                model: "claude-sonnet-5".into(),
                ingress: "messages".into(),
                egress: "messages".into(),
                status: "ok".into(),
                user_id: Some(user.clone()),
                ..RequestRow::default()
            })
            .unwrap();
    }

    let ana = harness.as_person(&harness.ana);
    let (_, listing) = harness.get("/api/requests", &ana).await;
    let ids: Vec<&str> = listing["requests"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|row| row["id"].as_str())
        .collect();
    assert_eq!(ids, ["req_ana"]);
    assert_eq!(harness.get("/api/requests/req_mino", &ana).await.0, 404);
}

#[tokio::test]
async fn a_member_issues_keys_only_for_themselves() {
    let harness = harness().await;
    let ana = harness.as_person(&harness.ana);
    let (status, _) = harness
        .post(
            "/api/keys",
            &ana,
            json!({ "user_id": harness.mino, "label": "pi" }),
        )
        .await;
    assert_eq!(status, 201);
    let keys = harness.store.keys().unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].user_id, harness.ana);
}

#[tokio::test]
async fn a_plan_belongs_to_a_person_and_never_to_everyone() {
    let harness = harness().await;
    let mino = harness.as_person(&harness.mino);
    let (status, refused) = harness
        .post(
            "/api/accounts",
            &mino,
            json!({ "name": "Shared Max", "provider": "anthropic", "owner": "everyone",
                    "credential_kind": "setup_token", "secret": "sk-ant-oat01-abc" }),
        )
        .await;
    assert_eq!(status, 400);
    assert!(
        refused["error"].as_str().unwrap().contains("person"),
        "{refused}"
    );

    let ana = harness.as_person(&harness.ana);
    let (status, _) = harness
        .post(
            "/api/accounts",
            &ana,
            json!({ "name": "Ana Max 2", "provider": "anthropic",
                    "credential_kind": "setup_token", "secret": "sk-ant-oat01-abc" }),
        )
        .await;
    assert_eq!(status, 201);
    let added = harness
        .store
        .accounts()
        .unwrap()
        .into_iter()
        .find(|account| account.name == "Ana Max 2")
        .unwrap();
    assert_eq!(added.owner.as_deref(), Some(harness.ana.as_str()));
    assert_eq!(added.credential_kind, "oauth");
}

#[tokio::test]
async fn signing_out_ends_the_session_on_the_server() {
    let harness = harness().await;
    let ana = harness.as_person(&harness.ana);
    assert_eq!(harness.get("/api/me", &ana).await.0, 200);
    reqwest::Client::new()
        .delete(harness.url("/api/session"))
        .header("cookie", &ana)
        .send()
        .await
        .unwrap();
    assert_eq!(harness.get("/api/me", &ana).await.0, 401);
}

#[tokio::test]
async fn the_admin_key_only_recovers_a_person() {
    let harness = harness().await;
    let open = |body: Value| {
        reqwest::Client::new()
            .post(harness.url("/api/session"))
            .json(&body)
            .send()
    };

    let alone = open(json!({ "key": ADMIN })).await.unwrap();
    assert_eq!(
        alone.status(),
        400,
        "a session that is nobody is what made every screen ask who you are"
    );

    let recovered = open(json!({ "key": ADMIN, "name": "ana", "password": "a new password" }))
        .await
        .unwrap();
    assert_eq!(recovered.status(), 204);
    let cookie = recovered.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let (_, me) = harness.get("/api/me", &cookie).await;
    assert_eq!(me["person"]["id"], harness.ana.as_str());
    assert_eq!(me["admin"], true);
    assert!(
        harness
            .store
            .signed_in("ana", "a new password")
            .unwrap()
            .is_some()
    );
}
