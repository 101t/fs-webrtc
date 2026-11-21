//! Everything FreeSWITCH-facing:
//!
//! * `POST /freeswitch/xml` — mod_xml_curl asks who may register (directory) and where a call
//!   goes (dialplan). Only users in the database exist, and only mutual contacts can call.
//! * `GET /sip` — relays browsers' SIP-over-WebSocket to FreeSWITCH, so the app, the API and SIP
//!   share one origin (one TLS certificate, one port) and FreeSWITCH stays on loopback.

use std::{collections::HashMap, time::Duration};

use axum::{
    Router,
    extract::{
        DefaultBodyLimit, Form, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::{self, client::IntoClientRequest};

use crate::api::{Shared, constant_time_eq, valid_username};

/// SIP domain of every account. Browsers reach FreeSWITCH through `/sip`, so it never needs to resolve.
pub const SIP_DOMAIN: &str = "fs-webrtc";

pub fn router(state: Shared) -> Router {
    Router::new()
        // FreeSWITCH posts every channel variable with dialplan lookups.
        .route("/freeswitch/xml", post(xml_curl).layer(DefaultBodyLimit::max(1024 * 1024)))
        .route("/sip", get(sip_websocket))
        .with_state(state)
}

const NOT_FOUND: &str =
    r#"<document type="freeswitch/xml"><section name="result"><result status="not found"/></section></document>"#;

async fn xml_curl(State(s): State<Shared>, headers: HeaderMap, Form(p): Form<HashMap<String, String>>) -> Response {
    let expected = format!("freeswitch:{}", s.cfg.freeswitch_secret);
    let given = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Basic "))
        .and_then(|b| STANDARD.decode(b.trim()).ok());
    if !given.is_some_and(|g| constant_time_eq(&g, expected.as_bytes())) {
        tracing::warn!("rejected /freeswitch/xml request with wrong credentials");
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let get = |k: &str| p.get(k).map(String::as_str).unwrap_or_default();
    let xml = match get("section") {
        "directory" => directory(&s, get("user"), get("domain")).await,
        "dialplan" => dialplan(&s, get("variable_sip_auth_username"), get("Caller-Destination-Number")).await,
        _ => Ok(NOT_FOUND.to_string()),
    };
    match xml {
        Ok(xml) => ([(header::CONTENT_TYPE, HeaderValue::from_static("text/xml"))], xml).into_response(),
        Err(e) => {
            tracing::error!("xml_curl {}: {e}", get("section"));
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// A SIP account, looked up on every REGISTER/INVITE challenge and when a call is bridged to `user/<name>`.
async fn directory(s: &Shared, user: &str, domain: &str) -> Result<String, sqlx::Error> {
    if domain != SIP_DOMAIN || !valid_username(user) {
        return Ok(NOT_FOUND.to_string());
    }
    let password: Option<String> = sqlx::query_scalar("SELECT sip_password FROM users WHERE username = ?")
        .bind(user)
        .fetch_optional(&s.db)
        .await?;
    // Only validated usernames and hex passwords are interpolated: nothing here needs XML escaping.
    Ok(password.map_or_else(
        || NOT_FOUND.to_string(),
        |password| {
            format!(
                r#"<document type="freeswitch/xml">
  <section name="directory">
    <domain name="{SIP_DOMAIN}">
      <params>
        <param name="dial-string" value="{{^^:sip_invite_domain=${{dialed_domain}}:presence_id=${{dialed_user}}@${{dialed_domain}}}}${{sofia_contact(*/${{dialed_user}}@${{dialed_domain}})}}"/>
      </params>
      <user id="{user}">
        <params>
          <param name="password" value="{password}"/>
        </params>
        <variables>
          <variable name="effective_caller_id_name" value="{user}"/>
          <variable name="effective_caller_id_number" value="{user}"/>
        </variables>
      </user>
    </domain>
  </section>
</document>"#
            )
        },
    ))
}

/// Routes an authenticated caller to a mutual contact; anything else is refused with 404.
/// `media_timeout` (ms, both legs) hangs up calls whose browser vanished: closed laptop, lost network.
async fn dialplan(s: &Shared, caller: &str, callee: &str) -> Result<String, sqlx::Error> {
    let allowed = valid_username(caller) && valid_username(callee) && caller != callee && {
        let rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM contacts c
             JOIN users a ON a.id = c.user_id JOIN users b ON b.id = c.contact_id
             WHERE (a.username = ?1 AND b.username = ?2) OR (a.username = ?2 AND b.username = ?1)",
        )
        .bind(caller)
        .bind(callee)
        .fetch_one(&s.db)
        .await?;
        rows == 2
    };
    tracing::info!(caller, callee, allowed, "call");
    let actions = if allowed {
        format!(
            r#"<action application="set" data="hangup_after_bridge=true"/>
          <action application="set" data="call_timeout=60"/>
          <action application="export" data="media_timeout=60000"/>
          <action application="bridge" data="user/{callee}@{SIP_DOMAIN}"/>"#
        )
    } else {
        r#"<action application="respond" data="404 Not a contact"/>"#.to_string()
    };
    Ok(format!(
        r#"<document type="freeswitch/xml">
  <section name="dialplan">
    <context name="webrtc">
      <extension name="fs-webrtc">
        <condition>
          {actions}
        </condition>
      </extension>
    </context>
  </section>
</document>"#
    ))
}

async fn sip_websocket(State(s): State<Shared>, ws: WebSocketUpgrade) -> Response {
    let upstream = s.cfg.freeswitch_ws.clone();
    ws.protocols(["sip"]).on_upgrade(move |client| async move {
        if let Err(e) = relay(client, &upstream).await {
            tracing::debug!("sip websocket closed: {e}");
        }
    })
}

async fn relay(client: WebSocket, upstream: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut req = upstream.into_client_request()?;
    req.headers_mut().insert(header::SEC_WEBSOCKET_PROTOCOL, HeaderValue::from_static("sip"));
    let (fs, _) = tokio_tungstenite::connect_async(req).await?;
    let (mut fs_tx, mut fs_rx) = fs.split();
    let (mut client_tx, mut client_rx) = client.split();

    let up = async {
        while let Some(msg) = client_rx.next().await {
            let msg = match msg? {
                Message::Text(t) => tungstenite::Message::text(t.as_str()),
                Message::Binary(b) => tungstenite::Message::binary(b),
                Message::Close(_) => break,
                Message::Ping(_) | Message::Pong(_) => continue, // answered by each side's library
            };
            fs_tx.send(msg).await?;
        }
        fs_tx.close().await?;
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
    };
    let down = async {
        while let Some(msg) = fs_rx.next().await {
            let msg = match msg? {
                tungstenite::Message::Text(t) => Message::text(t.as_str()),
                tungstenite::Message::Binary(b) => Message::binary(b),
                tungstenite::Message::Close(_) => break,
                _ => continue,
            };
            client_tx.send(msg).await?;
        }
        client_tx.close().await?;
        Ok(())
    };
    tokio::select! { r = up => r, r = down => r }
}

/// Whether something accepts TCP connections at the host and port of `url`.
pub async fn reachable(url: &str) -> bool {
    let Ok(uri) = url.parse::<Uri>() else { return false };
    let Some(host) = uri.host() else { return false };
    let port = uri.port_u16().unwrap_or(80);
    let connect = tokio::net::TcpStream::connect((host, port));
    matches!(tokio::time::timeout(Duration::from_secs(1), connect).await, Ok(Ok(_)))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use crate::api::tests::{SECRET, app, call, signup, xml_curl};

    #[tokio::test]
    async fn freeswitch_only_knows_users_and_routes_contacts() {
        let app = app(&[]).await;
        let (alice, bob) = (signup(&app, "alice").await, signup(&app, "bob").await);
        let dir = |user| [("section", "directory"), ("user", user), ("domain", "fs-webrtc"), ("action", "sip_auth")];

        assert_eq!(xml_curl(&app, &dir("alice"), "wrong-secret").await.0, StatusCode::UNAUTHORIZED);
        let (s, xml) = xml_curl(&app, &dir("alice"), SECRET).await;
        let (_, me) = call(&app, "GET", "/api/v1/me", Some(&alice), None).await;
        let password = me["sip"]["password"].as_str().unwrap();
        assert_eq!(s, StatusCode::OK);
        assert!(xml.contains(r#"<user id="alice">"#) && xml.contains(&format!(r#"value="{password}""#)), "{xml}");
        assert!(xml_curl(&app, &dir("carol"), SECRET).await.1.contains("not found"));
        let other_domain = [("section", "directory"), ("user", "alice"), ("domain", "evil.example")];
        assert!(xml_curl(&app, &other_domain, SECRET).await.1.contains("not found"));

        let dial = |from, to| {
            [("section", "dialplan"), ("variable_sip_auth_username", from), ("Caller-Destination-Number", to)]
        };
        assert!(xml_curl(&app, &dial("alice", "bob"), SECRET).await.1.contains("404 Not a contact"));
        call(&app, "POST", "/api/v1/contacts", Some(&alice), Some(json!({"username": "bob"}))).await;
        assert!(xml_curl(&app, &dial("alice", "bob"), SECRET).await.1.contains("404 Not a contact"), "request only");
        call(&app, "POST", "/api/v1/contacts", Some(&bob), Some(json!({"username": "alice"}))).await;
        let (_, xml) = xml_curl(&app, &dial("alice", "bob"), SECRET).await;
        assert!(xml.contains(r#"data="user/bob@fs-webrtc""#), "{xml}");
        assert!(xml_curl(&app, &dial("bob", "alice"), SECRET).await.1.contains("user/alice@fs-webrtc"));
        assert!(xml_curl(&app, &dial("", "bob"), SECRET).await.1.contains("404"), "unauthenticated caller");
        assert!(xml_curl(&app, &dial("alice", "bob%3Cx"), SECRET).await.1.contains("404"), "injection");
    }
}
