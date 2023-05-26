use actix_web::{
    dev::Payload, error::Error as AwError, http::header::HeaderMap, FromRequest, HttpRequest,
};

use futures::future::LocalBoxFuture;
use serde::Serialize;
use serde_json::json;

#[derive(Debug)]
pub struct IsHtmx(pub bool);

impl FromRequest for IsHtmx {
    type Error = AwError;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let req = req.clone();

        Box::pin(async move {
            if req.headers().contains_key("HX-Request") {
                Ok(Self(true))
            } else {
                Ok(Self(false))
            }
        })
    }
}

impl std::ops::Deref for IsHtmx {
    type Target = bool;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

macro_rules! bool_from_header {
    ($a:expr) => {
        match $a {
            Some(v) => match v.to_str() {
                Ok("true") => true,
                _ => false,
            },
            _ => false,
        }
    };
}

macro_rules! opt_string_from_header {
    ($a:expr) => {
        match $a {
            Some(v) => match v.to_str() {
                Ok(s) => Some(s.to_owned()),
                _ => None,
            },
            None => None,
        }
    };
}

#[derive(Debug, Clone)]
pub struct HxHeaderInfo {
    pub boosted: bool,
    pub current_url: Option<String>,
    pub history_restore_request: bool,
    pub prompt: Option<String>,
    pub target: Option<String>,
    pub trigger_name: Option<String>,
    pub trigger: Option<String>,
}

impl HxHeaderInfo {
    fn from_headers(headers: &HeaderMap) -> Self {
        let boosted = bool_from_header!(headers.get("HX-Boosted"));
        let current_url = opt_string_from_header!(headers.get("HX-Current-URL"));
        let history_restore_request = bool_from_header!(headers.get("HX-History-Restore-Request"));
        let prompt = opt_string_from_header!(headers.get("HX-Prompt"));
        let trigger_name = opt_string_from_header!(headers.get("HX-Trigger-Name"));
        let trigger = opt_string_from_header!(headers.get("HX-Trigger"));
        let target = opt_string_from_header!(headers.get("HX-Target"));

        Self {
            boosted,
            current_url,
            history_restore_request,
            prompt,
            target,
            trigger_name,
            trigger,
        }
    }
}

impl FromRequest for HxHeaderInfo {
    type Error = AwError;
    type Future = LocalBoxFuture<'static, Result<Self, Self::Error>>;
    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let info = Self::from_headers(req.clone().headers());

        Box::pin(async move { Ok(info) })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NotificationVariant {
    Success,
    Failure,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationEvent {
    title: String,
    message: String,
    variant: NotificationVariant,
    auto_hide: bool,
}

pub fn hx_trigger_notification(
    title: String,
    message: String,
    variant: NotificationVariant,
    auto_hide: bool,
) -> (String, String) {
    let event = json!({
        "notify": NotificationEvent {
            title,
            message,
            variant,
            auto_hide,
        },
    })
    .to_string();
    ("HX-Trigger-After-Swap".into(), event)
}
