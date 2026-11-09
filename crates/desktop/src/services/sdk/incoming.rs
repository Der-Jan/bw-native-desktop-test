//! Lightweight Vaultwarden revision checks. A checkpoint is captured BEFORE a
//! full download, so changes racing that download are seen by the next probe.
use super::{SessionClient, SessionToken, refresh};
use crate::services::accounts::{profile::ProfileStore, transport::json_body_limited};
use std::{path::PathBuf, sync::atomic::Ordering};
pub(super) async fn authorized_revision(
    session: &SessionToken,
    root: PathBuf,
    store: ProfileStore,
) -> Result<(refresh::Authorized, Option<i64>), String> {
    let mut authority = refresh::authorized(session, root.clone(), store.clone(), false).await?;
    for attempt in 0..2 {
        session.ensure_unlocked()?;
        let url = url::Url::parse(&authority.record.api)
            .and_then(|u| u.join("accounts/revision-date"))
            .map_err(|_| "Invalid revision endpoint")?;
        let response = authority
            .transport
            .execute(
                authority
                    .transport
                    .request(reqwest_account::Method::GET, url)?
                    .bearer_auth(&authority.credentials.access_token)
                    .build()
                    .map_err(|_| "Could not prepare incoming sync check")?,
            )
            .await?;
        session.ensure_unlocked()?;
        if response.status().as_u16() == 401 && attempt == 0 {
            authority = refresh::authorized(session, root.clone(), store.clone(), true).await?;
            continue;
        }
        if response.status().as_u16() == 404 {
            return Ok((authority, None));
        }
        if !response.status().is_success() {
            if response.status().as_u16() == 401 {
                session
                    .state
                    .network_authorized
                    .store(false, Ordering::SeqCst);
            }
            return Err("Incoming sync could not verify the server revision. Sign in again if the session expired.".into());
        }
        let value = json_body_limited(response, 256).await?;
        let revision = value
            .as_i64()
            .filter(|v| *v >= 0)
            .ok_or("Invalid server revision response")?;
        return Ok((authority, Some(revision)));
    }
    Err("Incoming sync session expired".into())
}
pub async fn changed(
    client: SessionClient,
    root: PathBuf,
    store: ProfileStore,
) -> Result<bool, String> {
    let session = client.session();
    client
        .run(move |_| async move {
            let (_, revision) = authorized_revision(&session, root, store).await?;
            let snapshot = session.state.snapshot.current();
            let before = snapshot.as_ref().and_then(|s| s.persisted.server_revision);
            // Policies and some metadata can change without a user revision bump.
            let overdue = snapshot.as_ref().is_none_or(|s| {
                chrono::Utc::now().signed_duration_since(s.persisted.downloaded_at)
                    >= chrono::Duration::minutes(5)
            });
            Ok(overdue || revision.is_none() || revision != before)
        })
        .await
}
