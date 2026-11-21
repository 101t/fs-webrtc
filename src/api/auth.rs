//! Sign-up, sign-in, sessions and the signed-in user's SIP account.

use std::sync::LazyLock;

use argon2::{Argon2, password_hash::PasswordHasher};
use axum::{
    Extension, Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{
    ApiError, ErrorBody, Shared, UserId, bearer, constant_time_eq, new_session, now, random, require, token_hash,
    valid_username,
};
use crate::freeswitch::SIP_DOMAIN;

#[derive(Serialize, ToSchema)]
pub struct ServerInfo {
    version: &'static str,
    /// Whether new accounts can be created.
    signup: bool,
    /// Whether creating an account needs an invite code.
    invite_required: bool,
}

/// What the sign-in page needs to know. No authentication.
#[utoipa::path(get, path = "/api/v1/info", tag = "auth", security(()), responses((status = OK, body = ServerInfo)))]
pub async fn info(State(s): State<Shared>) -> Json<ServerInfo> {
    Json(ServerInfo {
        version: env!("CARGO_PKG_VERSION"),
        signup: s.cfg.signup,
        invite_required: s.cfg.invite_code.is_some(),
    })
}

#[derive(Deserialize, ToSchema)]
pub struct RegisterRequest {
    /// 3-32 characters: `a-z 0-9 . _ -`, starting with a letter or digit. Case-insensitive; stored lowercase.
    #[schema(example = "alice")]
    username: String,
    /// 8-128 characters.
    #[schema(example = "correct horse battery")]
    password: String,
    /// Name shown to contacts; defaults to the username.
    #[schema(example = "Alice Liddell")]
    display_name: Option<String>,
    /// Required when the server is invite-only (see `/api/v1/info`).
    invite_code: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct LoginRequest {
    #[schema(example = "alice")]
    username: String,
    #[schema(example = "correct horse battery")]
    password: String,
}

#[derive(Serialize, ToSchema)]
pub struct Session {
    /// Bearer token, valid for 30 days.
    token: String,
    username: String,
    display_name: String,
}

/// Create an account and sign in.
#[utoipa::path(
    post, path = "/api/v1/auth/register", tag = "auth", security(()), request_body = RegisterRequest,
    responses(
        (status = CREATED, body = Session),
        (status = BAD_REQUEST, body = ErrorBody),
        (status = FORBIDDEN, description = "Sign-up closed or wrong invite code", body = ErrorBody),
        (status = CONFLICT, description = "Username taken", body = ErrorBody),
    )
)]
pub async fn register(
    State(s): State<Shared>,
    Json(req): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<Session>), ApiError> {
    if !s.cfg.signup {
        return Err(ApiError::Forbidden("sign-up is closed on this server"));
    }
    if let Some(code) = &s.cfg.invite_code {
        let given = req.invite_code.as_deref().unwrap_or_default().trim();
        if !constant_time_eq(given.as_bytes(), code.as_bytes()) {
            return Err(ApiError::Forbidden("a valid invite code is required"));
        }
    }
    let username = req.username.trim().to_ascii_lowercase();
    require(valid_username(&username), "username: 3-32 characters of a-z 0-9 . _ - starting with a letter or digit")?;
    require((8..=128).contains(&req.password.chars().count()), "password: 8-128 characters")?;
    let display_name = req.display_name.as_deref().map(str::trim).filter(|d| !d.is_empty()).unwrap_or(&username);
    require(
        display_name.chars().count() <= 64 && !display_name.chars().any(char::is_control),
        "display name: up to 64 characters",
    )?;
    let display_name = display_name.to_string();

    let password_hash = s.hash_password(req.password).await?;
    let sip_password: String = random::<16>().iter().map(|b| format!("{b:02x}")).collect();
    let id: Option<i64> = sqlx::query_scalar(
        "INSERT INTO users (username, display_name, password_hash, sip_password, created_at, last_seen)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5) ON CONFLICT (username) DO NOTHING RETURNING id",
    )
    .bind(&username)
    .bind(&display_name)
    .bind(password_hash)
    .bind(sip_password)
    .bind(now())
    .fetch_optional(&s.db)
    .await?;
    let id = id.ok_or(ApiError::Conflict("that username is taken"))?;
    tracing::info!(username, "account created");
    let token = new_session(&s.db, id).await?;
    Ok((StatusCode::CREATED, Json(Session { token, username, display_name })))
}

/// Hash checked when the username does not exist, so both failures take the same time.
static DUMMY_HASH: LazyLock<String> =
    LazyLock::new(|| Argon2::default().hash_password(b"no such user").expect("argon2 with default params").to_string());

/// Sign in.
#[utoipa::path(
    post, path = "/api/v1/auth/login", tag = "auth", security(()), request_body = LoginRequest,
    responses((status = OK, body = Session), (status = UNAUTHORIZED, body = ErrorBody))
)]
pub async fn login(State(s): State<Shared>, Json(req): Json<LoginRequest>) -> Result<Json<Session>, ApiError> {
    let username = req.username.trim().to_ascii_lowercase();
    let user: Option<(i64, String, String)> =
        sqlx::query_as("SELECT id, display_name, password_hash FROM users WHERE username = ?")
            .bind(&username)
            .fetch_optional(&s.db)
            .await?;
    let hash = user.as_ref().map_or_else(|| DUMMY_HASH.clone(), |u| u.2.clone());
    let ok = s.verify_password(req.password, hash).await?;
    match user {
        Some((id, display_name, _)) if ok => {
            let token = new_session(&s.db, id).await?;
            Ok(Json(Session { token, username, display_name }))
        }
        _ => Err(ApiError::Unauthorized("wrong username or password")),
    }
}

/// End this session.
#[utoipa::path(post, path = "/api/v1/auth/logout", tag = "auth", responses((status = NO_CONTENT)))]
pub async fn logout(State(s): State<Shared>, headers: HeaderMap) -> Result<StatusCode, ApiError> {
    if let Some(token) = bearer(&headers) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?").bind(token_hash(token)).execute(&s.db).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize, ToSchema)]
pub struct Me {
    username: String,
    display_name: String,
    sip: SipAccount,
}

/// Everything the browser needs to register with FreeSWITCH.
#[derive(Serialize, ToSchema)]
pub struct SipAccount {
    /// This user's SIP address.
    #[schema(example = "sip:alice@fs-webrtc")]
    uri: String,
    /// SIP domain; call `sip:<username>@<domain>`.
    domain: &'static str,
    /// SIP digest password, generated by the server.
    password: String,
    /// SIP-over-WebSocket path on this server.
    websocket_path: &'static str,
    /// STUN/TURN URLs for `RTCPeerConnection`.
    ice_servers: Vec<String>,
}

/// The signed-in user and their SIP credentials.
#[utoipa::path(get, path = "/api/v1/me", tag = "auth", responses((status = OK, body = Me)))]
pub async fn me(State(s): State<Shared>, Extension(UserId(id)): Extension<UserId>) -> Result<Json<Me>, ApiError> {
    let (username, display_name, password): (String, String, String) =
        sqlx::query_as("SELECT username, display_name, sip_password FROM users WHERE id = ?")
            .bind(id)
            .fetch_one(&s.db)
            .await?;
    let sip = SipAccount {
        uri: format!("sip:{username}@{SIP_DOMAIN}"),
        domain: SIP_DOMAIN,
        password,
        websocket_path: "/sip",
        ice_servers: s.cfg.ice_servers.clone(),
    };
    Ok(Json(Me { username, display_name, sip }))
}
