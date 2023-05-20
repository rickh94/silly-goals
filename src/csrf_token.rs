use std::fmt::Display;

use actix_session::Session;
use actix_web::error::ErrorInternalServerError;
use anyhow::anyhow;
use base64::{engine::general_purpose, Engine};
use log::error;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use crate::SessionValue;

#[derive(Eq, PartialEq, Serialize, Deserialize, Debug, Clone)]
pub struct CsrfToken(String);

impl CsrfToken {
    #[must_use]
    pub fn new() -> Self {
        let mut csrf_bytes = [0u8; 32];
        thread_rng().fill(&mut csrf_bytes);
        Self(general_purpose::STANDARD.encode(csrf_bytes))
    }

    #[must_use]
    pub fn verify(&self, other_token: &str) -> bool {
        self.0 == *other_token
    }

    pub fn get_or_create(session: &Session) -> actix_web::Result<Self> {
        if let Some(token) = Self::get(session).map_err(|err| {
            error!("CSRF error: {}", err);
            ErrorInternalServerError(err)
        })? {
            Ok(token)
        } else {
            let token = Self::new();
            token.save(session)?;
            Ok(token)
        }
    }

    pub fn verify_from_session(session: &Session, submitted_token: &str) -> actix_web::Result<()> {
        if let Ok(Some(correct_token)) = Self::get(session) {
            if correct_token.verify(submitted_token) {
                return Ok(());
            }
        }
        Err(ErrorInternalServerError(anyhow!(
            "Could not verify CSRF token from session"
        )))
    }
}

impl From<String> for CsrfToken {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for CsrfToken {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl SessionValue for CsrfToken {
    fn save_name() -> &'static str {
        "csrf_token"
    }
}

impl Default for CsrfToken {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for CsrfToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<input type=\"hidden\" value=\"{}\" name=\"csrftoken\">",
            self.0
        )
    }
}
