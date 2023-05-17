use core::fmt;
use std::{
    fmt::Display,
    ops::{Deref, Not},
};

use actix_session::{Session, SessionGetError};
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};

use crate::SessionValue;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct LoginCode(String);

impl LoginCode {
    pub fn new() -> Self {
        let num = thread_rng().gen_range(0..=999_999);
        Self(format!("{num:06}"))
    }

    pub fn verify(&self, other: &str) -> bool {
        *other == self.0
    }
}

impl SessionValue for LoginCode {
    fn save_name() -> &'static str {
        "login_code"
    }
}

impl Display for LoginCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct RegistrationEmail(String);

impl SessionValue for RegistrationEmail {
    fn save_name() -> &'static str {
        "registration_email"
    }
}

impl Deref for RegistrationEmail {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for RegistrationEmail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<T> From<T> for RegistrationEmail
where
    T: Into<String>,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct LoginEmail(String);

impl SessionValue for LoginEmail {
    fn save_name() -> &'static str {
        "login_email"
    }
}

impl Deref for LoginEmail {
    type Target = String;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> From<T> for LoginEmail
where
    T: Into<String>,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl Display for LoginEmail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct UsedWebauthn(bool);

impl SessionValue for UsedWebauthn {
    fn save_name() -> &'static str {
        "used_webauthn"
    }
}

impl Deref for UsedWebauthn {
    type Target = bool;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> From<T> for UsedWebauthn
where
    T: Into<bool>,
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl Display for UsedWebauthn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "used_webauthn: {}", self.0)
    }
}

impl UsedWebauthn {
    pub fn get_or_false(session: &Session) -> Result<Self, SessionGetError> {
        Ok(Self::get(session)?.unwrap_or(Self::from(false)))
    }
}

impl Not for UsedWebauthn {
    type Output = bool;
    fn not(self) -> Self::Output {
        !self.0
    }
}
