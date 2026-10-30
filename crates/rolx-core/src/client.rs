use crate::api::sign_in::{self, SignInExtensionError};
use crate::session::Session;
use crate::store::{SessionStore, StoreError};

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("no session found — log in first")]
    NoSession,

    #[error("session is expired — log in again")]
    Expired,

    #[error("failed to load session: {0}\n — try log in again")]
    Load(StoreError),

    #[error("failed to save the extended session: {0}")]
    Save(StoreError),

    #[error("failed to extend session: {0}\n — try log in again")]
    Extension(SignInExtensionError),
}

/// Returns a session usable right now: `session` itself, or, if near expiry, its extension, which
/// is also saved to `store`.
pub async fn ensure_fresh(
    http: &reqwest::Client,
    server_url: &str,
    session: Session,
    store: &dyn SessionStore,
) -> Result<Session, SessionError> {
    if session.is_expired() {
        return Err(SessionError::Expired);
    }
    if !session.is_near_expiry() {
        return Ok(session);
    }

    match sign_in::extend(http, server_url, &session.bearer_token).await {
        Ok(refreshed) => {
            store.save(&refreshed).map_err(SessionError::Save)?;
            Ok(refreshed)
        }
        Err(SignInExtensionError::SessionExpired) => Err(SessionError::Expired),
        Err(e) => Err(SessionError::Extension(e)),
    }
}

pub struct RolxClient {
    pub http: reqwest::Client,
    pub server_url: String,
    pub session: Session,
}

impl RolxClient {
    /// Client with the session of `store`, freshened as by [`ensure_fresh`].
    pub async fn from_store(
        http: reqwest::Client,
        server_url: String,
        store: &dyn SessionStore,
    ) -> Result<Self, SessionError> {
        let session = store
            .load()
            .map_err(SessionError::Load)?
            .ok_or(SessionError::NoSession)?;
        let session = ensure_fresh(&http, &server_url, session, store).await?;
        Ok(Self {
            http,
            server_url,
            session,
        })
    }
}
