//! Mutual contacts. Adding someone sends a request; when they add you back, you can call each other.

use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use utoipa::ToSchema;

use super::{ApiError, ErrorBody, Shared, UserId, now, require, valid_username};

#[derive(Serialize, Deserialize, ToSchema, sqlx::Type, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
#[sqlx(rename_all = "lowercase")]
pub enum ContactStatus {
    /// You added each other: you can call.
    Accepted,
    /// You sent a request they have not accepted yet.
    Outgoing,
    /// They want to add you; add them back to accept.
    Incoming,
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Contact {
    #[schema(example = "bob")]
    username: String,
    #[schema(example = "Bob")]
    display_name: String,
    status: ContactStatus,
    /// Had the app open in the last minute. Always false unless `accepted`.
    online: bool,
}

async fn contacts(db: &SqlitePool, me: i64, only: Option<&str>) -> Result<Vec<Contact>, sqlx::Error> {
    sqlx::query_as(
        "SELECT u.username, u.display_name,
                CASE WHEN o.user_id IS NOT NULL AND i.user_id IS NOT NULL THEN 'accepted'
                     WHEN o.user_id IS NOT NULL THEN 'outgoing' ELSE 'incoming' END AS status,
                (o.user_id IS NOT NULL AND i.user_id IS NOT NULL AND u.last_seen > ?2 - 60) AS online
         FROM users u
         LEFT JOIN contacts o ON o.user_id = ?1 AND o.contact_id = u.id
         LEFT JOIN contacts i ON i.user_id = u.id AND i.contact_id = ?1
         WHERE (o.user_id IS NOT NULL OR i.user_id IS NOT NULL) AND (?3 IS NULL OR u.username = ?3)
         ORDER BY u.display_name COLLATE NOCASE",
    )
    .bind(me)
    .bind(now())
    .bind(only)
    .fetch_all(db)
    .await
}

/// Your contacts and pending requests, both ways.
#[utoipa::path(get, path = "/api/v1/contacts", tag = "contacts", responses((status = OK, body = Vec<Contact>)))]
pub async fn list(
    State(s): State<Shared>,
    Extension(UserId(me)): Extension<UserId>,
) -> Result<Json<Vec<Contact>>, ApiError> {
    Ok(Json(contacts(&s.db, me, None).await?))
}

#[derive(Deserialize, ToSchema)]
pub struct AddContact {
    #[schema(example = "bob")]
    username: String,
}

/// Send a contact request, or accept one by adding the sender back.
#[utoipa::path(
    post, path = "/api/v1/contacts", tag = "contacts", request_body = AddContact,
    responses((status = OK, body = Contact), (status = BAD_REQUEST, body = ErrorBody), (status = NOT_FOUND, body = ErrorBody))
)]
pub async fn add(
    State(s): State<Shared>,
    Extension(UserId(me)): Extension<UserId>,
    Json(req): Json<AddContact>,
) -> Result<Json<Contact>, ApiError> {
    let username = req.username.trim().trim_start_matches('@').to_ascii_lowercase();
    require(valid_username(&username), "that is not a valid username")?;
    let target: i64 = sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
        .bind(&username)
        .fetch_optional(&s.db)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("nobody is called {username} here")))?;
    require(target != me, "you cannot add yourself")?;
    sqlx::query("INSERT OR IGNORE INTO contacts (user_id, contact_id, created_at) VALUES (?, ?, ?)")
        .bind(me)
        .bind(target)
        .bind(now())
        .execute(&s.db)
        .await?;
    let contact = contacts(&s.db, me, Some(&username)).await?.pop();
    contact.map(Json).ok_or_else(|| ApiError::Internal("contact vanished after insert".into()))
}

/// Remove a contact, cancel your request, or decline theirs. Either way, neither side can call the other.
#[utoipa::path(
    delete, path = "/api/v1/contacts/{username}", tag = "contacts",
    params(("username" = String, Path)), responses((status = NO_CONTENT))
)]
pub async fn remove(
    State(s): State<Shared>,
    Extension(UserId(me)): Extension<UserId>,
    Path(username): Path<String>,
) -> Result<StatusCode, ApiError> {
    sqlx::query(
        "DELETE FROM contacts WHERE (user_id = ?1 AND contact_id = (SELECT id FROM users WHERE username = ?2))
                                 OR (contact_id = ?1 AND user_id = (SELECT id FROM users WHERE username = ?2))",
    )
    .bind(me)
    .bind(username.to_ascii_lowercase())
    .execute(&s.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use crate::api::tests::{app, call, signup};

    #[tokio::test]
    async fn requests_become_contacts_only_when_mutual() {
        let app = app(&[]).await;
        let (alice, bob) = (signup(&app, "alice").await, signup(&app, "bob").await);
        let add = |who: &str| Some(json!({"username": who}));

        assert_eq!(call(&app, "POST", "/api/v1/contacts", Some(&alice), add("alice")).await.0, StatusCode::BAD_REQUEST);
        assert_eq!(call(&app, "POST", "/api/v1/contacts", Some(&alice), add("carol")).await.0, StatusCode::NOT_FOUND);

        let (s, c) = call(&app, "POST", "/api/v1/contacts", Some(&alice), add("@Bob")).await;
        assert_eq!((s, c["status"].as_str(), c["online"].as_bool()), (StatusCode::OK, Some("outgoing"), Some(false)));
        let (_, list) = call(&app, "GET", "/api/v1/contacts", Some(&bob), None).await;
        assert_eq!(
            list,
            json!([{"username": "alice", "display_name": "ALICE", "status": "incoming", "online": false}])
        );

        call(&app, "POST", "/api/v1/contacts", Some(&bob), add("alice")).await;
        let (_, list) = call(&app, "GET", "/api/v1/contacts", Some(&alice), None).await;
        assert_eq!(list, json!([{"username": "bob", "display_name": "BOB", "status": "accepted", "online": true}]));

        assert_eq!(call(&app, "DELETE", "/api/v1/contacts/bob", Some(&alice), None).await.0, StatusCode::NO_CONTENT);
        for token in [&alice, &bob] {
            assert_eq!(call(&app, "GET", "/api/v1/contacts", Some(token), None).await.1, json!([]));
        }
    }
}
