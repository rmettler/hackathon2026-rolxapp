use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::clock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BearerToken(String);

impl BearerToken {
    pub fn from_string(s: String) -> Self {
        BearerToken(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub bearer_token: BearerToken,
    pub expires: DateTime<Utc>,
    pub user_id: String,
    pub user_email: String,
    pub user_first_name: String,
    pub user_last_name: String,
}

impl Session {
    pub fn is_near_expiry(&self) -> bool {
        self.expires < clock::now() + Duration::minutes(10)
    }

    pub fn is_expired(&self) -> bool {
        self.expires < clock::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expiring_in(lifetime: Duration) -> Session {
        Session {
            bearer_token: BearerToken::from_string("fake".into()),
            expires: clock::now() + lifetime,
            user_id: "id".into(),
            user_email: "test@example.invalid".into(),
            user_first_name: "Test".into(),
            user_last_name: "User".into(),
        }
    }

    #[test]
    fn long_lived_session_is_neither_near_expiry_nor_expired() {
        let session = expiring_in(Duration::hours(1));
        assert!(!session.is_near_expiry());
        assert!(!session.is_expired());
    }

    #[test]
    fn session_within_ten_minutes_is_near_expiry_but_not_expired() {
        let session = expiring_in(Duration::minutes(9));
        assert!(session.is_near_expiry());
        assert!(!session.is_expired());
    }

    #[test]
    fn past_session_is_near_expiry_and_expired() {
        let session = expiring_in(Duration::seconds(-1));
        assert!(session.is_near_expiry());
        assert!(session.is_expired());
    }
}
