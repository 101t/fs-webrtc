//! HTTP API: routing, authentication, errors, validation and the OpenAPI document.

mod auth;
mod contacts;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Request, State},
    http::{HeaderName, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tokio::sync::Semaphore;
use tower_http::{
    sensitive_headers::SetSensitiveRequestHeadersLayer,
    services::ServeDir,
    set_header::{SetResponseHeader, SetResponseHeaderLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use utoipa::{
    Modify, OpenApi, ToSchema,
    openapi::security::{Http, HttpAuthScheme, SecurityScheme},
};
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_scalar::{Scalar, Servable};

use crate::{config::Config, freeswitch};

pub struct AppState {
    pub cfg: Config,
    pub db: SqlitePool,
    /// Caps concurrent argon2 runs (~19 MiB and tens of ms each) so login floods cannot exhaust the host.
    hashing: Semaphore,
    /// Wrong-password count per user id and when that count started, for [`AppState::check_password`].
    failures: Mutex<HashMap<i64, (u32, i64)>>,
}

pub type Shared = Arc<AppState>;

const SESSION_TTL_SECS: i64 = 30 * 24 * 3600;
/// Wrong passwords allowed per account within [`LOCKOUT_SECS`] before it is locked for the rest of that window.
const MAX_FAILURES: u32 = 5;
const LOCKOUT_SECS: i64 = 15 * 60;

impl AppState {
    pub fn new(cfg: Config, db: SqlitePool) -> Self {
        Self { cfg, db, hashing: Semaphore::new(4), failures: Mutex::default() }
    }

    async fn hash_password(&self, password: String) -> Result<String, ApiError> {
        let _permit = self.hashing.acquire().await.map_err(|e| ApiError::Internal(e.to_string()))?;
        tokio::task::spawn_blocking(move || Argon2::default().hash_password(password.as_bytes()).map(|h| h.to_string()))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?
            .map_err(|e| ApiError::Internal(format!("argon2: {e}")))
    }

    async fn verify_password(&self, password: String, hash: String) -> Result<bool, ApiError> {
        let _permit = self.hashing.acquire().await.map_err(|e| ApiError::Internal(e.to_string()))?;
        tokio::task::spawn_blocking(move || {
            Argon2::default().verify_password(password.as_bytes(), hash.as_str()).is_ok()
        })
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))
    }

    /// Checks a password against a user's hash, allowing [`MAX_FAILURES`] wrong guesses per account per
    /// [`LOCKOUT_SECS`]. `id` is `None` for unknown users (checked against a dummy hash, never counted).
    async fn check_password(&self, id: Option<i64>, password: String, hash: String) -> Result<bool, ApiError> {
        if let Some(id) = id {
            let mut failures = self.failures.lock().expect("failures lock");
            let now = now();
            let (count, since) = failures.entry(id).or_insert((0, now));
            if now - *since >= LOCKOUT_SECS {
                (*count, *since) = (0, now);
            }
            if *count >= MAX_FAILURES {
                let minutes = (*since + LOCKOUT_SECS - now + 59) / 60;
                return Err(ApiError::TooManyRequests(format!(
                    "too many wrong passwords for this account, try again in {minutes} minute{}",
                    if minutes == 1 { "" } else { "s" }
                )));
            }
            // Counted before checking, so parallel guesses cannot all slip in under the limit.
            *count += 1;
        }
        let ok = self.verify_password(password, hash).await?;
        if let (Some(id), true) = (id, ok) {
            self.failures.lock().expect("failures lock").remove(&id);
        }
        Ok(ok)
    }

    /// Re-confirms the signed-in user's password before a sensitive change (throttled like sign-in).
    async fn confirm_password(&self, id: i64, password: String) -> Result<(), ApiError> {
        let hash: String =
            sqlx::query_scalar("SELECT password_hash FROM users WHERE id = ?").bind(id).fetch_one(&self.db).await?;
        if self.check_password(Some(id), password, hash).await? {
            Ok(())
        } else {
            // Not 401: that would sign the browser out.
            Err(ApiError::Forbidden("wrong password"))
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "fs-webrtc",
        description = "Accounts and contacts for fs-webrtc. Calls themselves run over SIP (WebSocket at `/sip`) \
                       to FreeSWITCH, which asks this server who may register and who may call whom.\n\n\
                       Authenticate with `Authorization: Bearer <token>` from `/api/v1/auth/login`."
    ),
    modifiers(&BearerAuth),
    security(("token" = [])),
    tags(
        (name = "auth", description = "Accounts and sessions"),
        (name = "contacts", description = "Mutual contacts: only accepted contacts can call each other"),
        (name = "system", description = "Health and metadata"),
    )
)]
struct ApiDoc;

struct BearerAuth;

impl Modify for BearerAuth {
    fn modify(&self, api: &mut utoipa::openapi::OpenApi) {
        api.components
            .get_or_insert_with(Default::default)
            .add_security_scheme("token", SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)));
    }
}

pub fn router(state: AppState) -> Router {
    let web = ServeDir::new(&state.cfg.web_dir);
    // The app loads nothing from other origins; /docs (Scalar, from a CDN) is outside this policy.
    let web = SetResponseHeader::overriding(
        web,
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; connect-src 'self'; img-src 'self' data:; media-src 'self' blob: mediastream:; \
             frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
    );
    let web = SetResponseHeader::overriding(
        web,
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(self), microphone=(self), geolocation=()"),
    );
    let state = Arc::new(state);

    let (api, openapi) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(auth::me, auth::delete_account))
        .routes(routes!(auth::change_password))
        .routes(routes!(auth::logout))
        .routes(routes!(contacts::list, contacts::add))
        .routes(routes!(contacts::remove))
        .route_layer(middleware::from_fn_with_state(state.clone(), authenticate))
        // Added after the auth layer, so these stay public.
        .routes(routes!(auth::info))
        .routes(routes!(auth::register))
        .routes(routes!(auth::login))
        .routes(routes!(health))
        .with_state(state.clone())
        .split_for_parts();

    api.layer(SetResponseHeaderLayer::if_not_present(header::CACHE_CONTROL, HeaderValue::from_static("no-store")))
        .merge(Scalar::with_url("/docs", openapi.clone()))
        .route("/openapi.json", get(move || async move { Json(openapi) }))
        .merge(freeswitch::router(state))
        .fallback_service(web)
        .layer(DefaultBodyLimit::max(64 * 1024))
        .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, Duration::from_secs(30)))
        .layer(SetResponseHeaderLayer::overriding(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
        .layer(SetResponseHeaderLayer::overriding(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer")))
        .layer(TraceLayer::new_for_http())
        .layer(SetSensitiveRequestHeadersLayer::new([header::AUTHORIZATION]))
}

/// The signed-in user, set by [`authenticate`].
#[derive(Clone, Copy)]
pub struct UserId(pub i64);

async fn authenticate(State(s): State<Shared>, mut req: Request, next: Next) -> Result<Response, ApiError> {
    let token = bearer(req.headers()).ok_or(ApiError::Unauthorized("missing session token"))?;
    let now = now();
    let id: i64 = sqlx::query_scalar("SELECT user_id FROM sessions WHERE token_hash = ? AND expires_at > ?")
        .bind(token_hash(token))
        .bind(now)
        .fetch_optional(&s.db)
        .await?
        .ok_or(ApiError::Unauthorized("session expired, sign in again"))?;
    // Drives the "online" dot that contacts see; the app polls while open.
    sqlx::query("UPDATE users SET last_seen = ?1 WHERE id = ?2 AND last_seen < ?1 - 5")
        .bind(now)
        .bind(id)
        .execute(&s.db)
        .await?;
    req.extensions_mut().insert(UserId(id));
    Ok(next.run(req).await)
}

fn bearer(headers: &header::HeaderMap) -> Option<&str> {
    headers.get(header::AUTHORIZATION)?.to_str().ok()?.strip_prefix("Bearer ").map(str::trim)
}

async fn new_session(db: &SqlitePool, user_id: i64) -> Result<String, ApiError> {
    let token = URL_SAFE_NO_PAD.encode(random::<32>());
    sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at) VALUES (?, ?, ?)")
        .bind(token_hash(&token))
        .bind(user_id)
        .bind(now() + SESSION_TTL_SECS)
        .execute(db)
        .await?;
    Ok(token)
}

fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub fn random<const N: usize>() -> [u8; N] {
    let mut buf = [0; N];
    getrandom::fill(&mut buf).expect("OS random number generator");
    buf
}

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Compares without an early exit, so response timing does not reveal how much of a secret matched.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[derive(Serialize, ToSchema)]
struct Health {
    /// Always `ok` while the process serves requests.
    status: &'static str,
    /// Whether FreeSWITCH's SIP WebSocket port accepts connections.
    freeswitch_reachable: bool,
    version: &'static str,
}

/// Liveness and FreeSWITCH reachability. No authentication.
#[utoipa::path(get, path = "/health", tag = "system", security(()), responses((status = OK, body = Health)))]
async fn health(State(s): State<Shared>) -> Json<Health> {
    let freeswitch_reachable = freeswitch::reachable(&s.cfg.freeswitch_ws).await;
    Json(Health { status: "ok", freeswitch_reachable, version: env!("CARGO_PKG_VERSION") })
}

// ---------------------------------------------------------------- validation

/// Usernames double as SIP user names and reach FreeSWITCH XML and dial strings, so: 3-32 of
/// `a-z 0-9 . _ -`, starting with a letter or digit. Nothing that could break out of either.
pub fn valid_username(s: &str) -> bool {
    (3..=32).contains(&s.len())
        && s.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

pub fn valid_password(s: &str) -> bool {
    (8..=128).contains(&s.chars().count())
}

pub fn require(ok: bool, msg: impl Into<String>) -> Result<(), ApiError> {
    if ok { Ok(()) } else { Err(ApiError::BadRequest(msg.into())) }
}

// ---------------------------------------------------------------- errors

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(&'static str),
    #[error("{0}")]
    Forbidden(&'static str),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(&'static str),
    #[error("{0}")]
    TooManyRequests(String),
    #[error("internal error")]
    Internal(String),
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::Internal(format!("database: {e}"))
    }
}

/// Every error response has this shape.
#[derive(Serialize, ToSchema)]
pub struct ErrorBody {
    /// Stable machine-readable code, e.g. `bad_request`, `unauthorized`.
    pub error: &'static str,
    /// Human-readable explanation.
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            Self::TooManyRequests(_) => (StatusCode::TOO_MANY_REQUESTS, "too_many_requests"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        if status.is_server_error() {
            tracing::error!(error = ?self, "request failed");
        }
        let message = match &self {
            Self::Internal(_) => "internal error, see server logs".to_string(),
            e => e.to_string(),
        };
        let mut res = (status, Json(ErrorBody { error: code, message })).into_response();
        if status == StatusCode::UNAUTHORIZED {
            res.headers_mut().insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        res
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use axum::body::Body;
    use base64::engine::general_purpose::STANDARD;
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    pub const SECRET: &str = "test-secret-0123456789abcdef0123456789";

    pub async fn app(extra: &[&str]) -> Router {
        let mut args = vec!["fs-webrtc", "--freeswitch-secret", SECRET, "--freeswitch-ws", "ws://127.0.0.1:1"];
        args.extend_from_slice(extra);
        let mut cfg = <Config as clap::Parser>::parse_from(args);
        cfg.validate().unwrap();
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                <sqlx::sqlite::SqliteConnectOptions as std::str::FromStr>::from_str("sqlite::memory:")
                    .unwrap()
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        router(AppState::new(cfg, db))
    }

    pub async fn send(app: &Router, req: Request) -> (StatusCode, Value, String) {
        let res = app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8_lossy(&bytes).into_owned();
        (status, serde_json::from_str(&text).unwrap_or(Value::Null), text)
    }

    pub async fn call(
        app: &Router,
        method: &str,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut req = Request::builder().method(method).uri(uri);
        if let Some(t) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        let body = match body {
            Some(b) => {
                req = req.header(header::CONTENT_TYPE, "application/json");
                Body::from(b.to_string())
            }
            None => Body::empty(),
        };
        let (s, v, _) = send(app, req.body(body).unwrap()).await;
        (s, v)
    }

    /// Registers a user and returns its session token.
    pub async fn signup(app: &Router, username: &str) -> String {
        let body = json!({"username": username, "password": "correct horse", "display_name": username.to_uppercase()});
        let (s, v) = call(app, "POST", "/api/v1/auth/register", None, Some(body)).await;
        assert_eq!(s, StatusCode::CREATED, "{v}");
        v["token"].as_str().unwrap().to_string()
    }

    /// Posts a mod_xml_curl request the way FreeSWITCH does.
    pub async fn xml_curl(app: &Router, form: &[(&str, &str)], secret: &str) -> (StatusCode, String) {
        let body = form.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&");
        let req = Request::post("/freeswitch/xml")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .header(header::AUTHORIZATION, format!("Basic {}", STANDARD.encode(format!("freeswitch:{secret}"))))
            .body(Body::from(body))
            .unwrap();
        let (s, _, text) = send(app, req).await;
        (s, text)
    }

    #[tokio::test]
    async fn accounts_and_sessions() {
        let app = app(&[]).await;
        let (s, v) = call(&app, "GET", "/health", None, None).await;
        assert_eq!((s, v["freeswitch_reachable"].as_bool()), (StatusCode::OK, Some(false)));
        assert_eq!(call(&app, "GET", "/api/v1/me", None, None).await.0, StatusCode::UNAUTHORIZED);
        assert_eq!(call(&app, "GET", "/api/v1/me", Some("bogus"), None).await.0, StatusCode::UNAUTHORIZED);

        let token = signup(&app, "alice").await;
        let (s, me) = call(&app, "GET", "/api/v1/me", Some(&token), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!((me["username"].as_str(), me["display_name"].as_str()), (Some("alice"), Some("ALICE")));
        assert_eq!(me["sip"]["uri"], "sip:alice@fs-webrtc");
        assert_eq!(me["sip"]["password"].as_str().unwrap().len(), 32);

        let dup = json!({"username": "Alice", "password": "another password"});
        assert_eq!(call(&app, "POST", "/api/v1/auth/register", None, Some(dup)).await.0, StatusCode::CONFLICT);
        for bad in [json!({"username": "a", "password": "longenough"}), json!({"username": "bob", "password": "short"})]
        {
            assert_eq!(call(&app, "POST", "/api/v1/auth/register", None, Some(bad)).await.0, StatusCode::BAD_REQUEST);
        }

        let login = |pw: &str| json!({"username": " ALICE ", "password": pw});
        assert_eq!(
            call(&app, "POST", "/api/v1/auth/login", None, Some(login("wrong pass"))).await.0,
            StatusCode::UNAUTHORIZED
        );
        let wrong_user = json!({"username": "nobody", "password": "correct horse"});
        assert_eq!(call(&app, "POST", "/api/v1/auth/login", None, Some(wrong_user)).await.0, StatusCode::UNAUTHORIZED);
        let (s, v) = call(&app, "POST", "/api/v1/auth/login", None, Some(login("correct horse"))).await;
        assert_eq!(s, StatusCode::OK);
        let second = v["token"].as_str().unwrap();

        assert_eq!(call(&app, "POST", "/api/v1/auth/logout", Some(second), None).await.0, StatusCode::NO_CONTENT);
        assert_eq!(call(&app, "GET", "/api/v1/me", Some(second), None).await.0, StatusCode::UNAUTHORIZED);
        assert_eq!(call(&app, "GET", "/api/v1/me", Some(&token), None).await.0, StatusCode::OK);

        let (s, spec) = call(&app, "GET", "/openapi.json", None, None).await;
        assert_eq!(s, StatusCode::OK);
        assert!(spec["paths"]["/api/v1/contacts"].is_object());
    }

    #[tokio::test]
    async fn signup_can_be_closed_or_invite_only() {
        let closed = app(&["--signup", "false"]).await;
        let (_, info) = call(&closed, "GET", "/api/v1/info", None, None).await;
        assert_eq!(info["signup"], false);
        let body = json!({"username": "alice", "password": "correct horse"});
        assert_eq!(call(&closed, "POST", "/api/v1/auth/register", None, Some(body)).await.0, StatusCode::FORBIDDEN);

        // `FSW_INVITE_CODE=` in an env file must not turn on invite-only mode.
        let (_, info) = call(&app(&["--invite-code", ""]).await, "GET", "/api/v1/info", None, None).await;
        assert_eq!(info["invite_required"], false);

        let invite = app(&["--invite-code", "family"]).await;
        let (_, info) = call(&invite, "GET", "/api/v1/info", None, None).await;
        assert_eq!((info["signup"].as_bool(), info["invite_required"].as_bool()), (Some(true), Some(true)));
        let body = json!({"username": "alice", "password": "correct horse", "invite_code": "nope"});
        assert_eq!(call(&invite, "POST", "/api/v1/auth/register", None, Some(body)).await.0, StatusCode::FORBIDDEN);
        let body = json!({"username": "alice", "password": "correct horse", "invite_code": "family"});
        assert_eq!(call(&invite, "POST", "/api/v1/auth/register", None, Some(body)).await.0, StatusCode::CREATED);
    }

    #[tokio::test]
    async fn wrong_passwords_lock_the_account() {
        let app = app(&[]).await;
        signup(&app, "alice").await;
        signup(&app, "bob").await;
        let login = |user: &str, pw: &str| json!({"username": user, "password": pw});
        for _ in 0..MAX_FAILURES {
            let (s, _) = call(&app, "POST", "/api/v1/auth/login", None, Some(login("alice", "wrong pass"))).await;
            assert_eq!(s, StatusCode::UNAUTHORIZED);
        }
        let (s, v) = call(&app, "POST", "/api/v1/auth/login", None, Some(login("alice", "correct horse"))).await;
        assert_eq!((s, v["error"].as_str()), (StatusCode::TOO_MANY_REQUESTS, Some("too_many_requests")), "{v}");
        assert!(v["message"].as_str().unwrap().contains("15 minutes"), "{v}");
        // Other accounts and unknown usernames are unaffected.
        assert_eq!(
            call(&app, "POST", "/api/v1/auth/login", None, Some(login("bob", "correct horse"))).await.0,
            StatusCode::OK
        );
        for _ in 0..=MAX_FAILURES {
            let (s, _) = call(&app, "POST", "/api/v1/auth/login", None, Some(login("nobody", "wrong pass"))).await;
            assert_eq!(s, StatusCode::UNAUTHORIZED);
        }
    }

    #[tokio::test]
    async fn password_change_and_account_deletion() {
        let app = app(&[]).await;
        let (alice, bob) = (signup(&app, "alice").await, signup(&app, "bob").await);
        let (_, v) = call(
            &app,
            "POST",
            "/api/v1/auth/login",
            None,
            Some(json!({"username": "alice", "password": "correct horse"})),
        )
        .await;
        let other_device = v["token"].as_str().unwrap().to_string();
        let sip = |t: String| {
            let app = app.clone();
            async move { call(&app, "GET", "/api/v1/me", Some(&t), None).await.1["sip"]["password"].clone() }
        };
        let old_sip = sip(alice.clone()).await;

        let change = |current: &str, new: &str| json!({"current_password": current, "new_password": new});
        let put = |body| call(&app, "PUT", "/api/v1/me/password", Some(&alice), Some(body));
        assert_eq!(put(change("wrong pass", "new password")).await.0, StatusCode::FORBIDDEN);
        assert_eq!(put(change("correct horse", "short")).await.0, StatusCode::BAD_REQUEST);
        assert_eq!(put(change("correct horse", "new password")).await.0, StatusCode::NO_CONTENT);
        assert_eq!(call(&app, "GET", "/api/v1/me", Some(&other_device), None).await.0, StatusCode::UNAUTHORIZED);
        let new_sip = sip(alice.clone()).await;
        assert!(new_sip.is_string() && new_sip != old_sip, "SIP password rotated");
        let login = |pw: &str| json!({"username": "alice", "password": pw});
        assert_eq!(
            call(&app, "POST", "/api/v1/auth/login", None, Some(login("correct horse"))).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(call(&app, "POST", "/api/v1/auth/login", None, Some(login("new password"))).await.0, StatusCode::OK);

        call(&app, "POST", "/api/v1/contacts", Some(&alice), Some(json!({"username": "bob"}))).await;
        call(&app, "POST", "/api/v1/contacts", Some(&bob), Some(json!({"username": "alice"}))).await;
        let delete = |pw: &str| call(&app, "DELETE", "/api/v1/me", Some(&alice), Some(json!({"password": pw})));
        assert_eq!(delete("correct horse").await.0, StatusCode::FORBIDDEN);
        assert_eq!(delete("new password").await.0, StatusCode::NO_CONTENT);
        assert_eq!(call(&app, "GET", "/api/v1/me", Some(&alice), None).await.0, StatusCode::UNAUTHORIZED);
        assert_eq!(call(&app, "GET", "/api/v1/contacts", Some(&bob), None).await.1, json!([]));
        let dir = [("section", "directory"), ("user", "alice"), ("domain", "fs-webrtc")];
        assert!(xml_curl(&app, &dir, SECRET).await.1.contains("not found"));
        signup(&app, "alice").await;
    }

    #[tokio::test]
    async fn turn_credentials_go_only_to_turn_servers() {
        let app = app(&[
            "--ice-servers",
            "stun:stun.example:3478,turn:turn.example:3478?transport=udp,turns:turn.example:5349",
            "--turn-username",
            "fsw",
            "--turn-password",
            "s3cret",
        ])
        .await;
        let token = signup(&app, "alice").await;
        let (_, me) = call(&app, "GET", "/api/v1/me", Some(&token), None).await;
        assert_eq!(
            me["sip"]["ice_servers"],
            json!([
                {"urls": ["stun:stun.example:3478"]},
                {"urls": ["turn:turn.example:3478?transport=udp", "turns:turn.example:5349"], "username": "fsw", "credential": "s3cret"},
            ])
        );
        let mut half = <Config as clap::Parser>::parse_from([
            "fs-webrtc",
            "--freeswitch-secret",
            SECRET,
            "--turn-username",
            "fsw",
        ]);
        assert!(half.validate().is_err());
    }

    #[test]
    fn usernames_cannot_inject() {
        assert!(valid_username("alice") && valid_username("bob.smith_2") && valid_username("007"));
        for bad in ["al", "-alice", ".alice", "Alice", "a b c", "a@b.c", "x${system}", "a/b", "a<b>", &"a".repeat(33)] {
            assert!(!valid_username(bad), "{bad}");
        }
    }
}
