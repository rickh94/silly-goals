pub mod csrf_token;
pub mod htmx;
pub mod mail;
pub mod queries;
pub mod routes;
pub mod session_values;
pub mod templates;

use std::str::FromStr;

use actix_session::Session;
use actix_web::{
    dev,
    error::ErrorInternalServerError,
    http::{header, StatusCode},
    middleware::ErrorHandlerResponse,
};
use anyhow::{anyhow, Result};
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{
    types::{Json, Uuid},
    QueryBuilder, Sqlite, SqlitePool,
};
use webauthn_rs::prelude::PasskeyRegistration;

pub trait SessionValue: Clone + Serialize + for<'a> Deserialize<'a> {
    fn save(&self, session: &Session) -> actix_web::Result<()> {
        session
            .insert(Self::save_name(), self.clone())
            .map_err(|err| {
                error!("Session error: {}", err);
                ErrorInternalServerError(err)
            })?;
        Ok(())
    }

    fn get(session: &Session) -> Result<Option<Self>> {
        Ok(session.get::<Self>(Self::save_name())?)
    }

    fn save_name() -> &'static str;

    fn get_some_or_err(session: &Session) -> Result<Self> {
        Self::get(session)?.ok_or(anyhow!("Could not get from session"))
    }

    fn remove(session: &Session) -> Option<String> {
        session.remove(Self::save_name())
    }
}

impl SessionValue for PasskeyRegistration {
    fn save_name() -> &'static str {
        "reg_stage"
    }
}

#[derive(Clone, Debug)]
pub struct User {
    pub id: i64,
    pub name: Option<String>,
    pub email: String,
    pub userid: Uuid,
    pub is_new_user: bool,
}

#[derive(sqlx::Type, Debug, Clone)]
#[sqlx(type_name = "goal_behavior")]
#[sqlx(rename_all = "lowercase")]
pub enum GoalBehavior {
    Hide,
    Nice,
    Mean,
}

#[derive(sqlx::Type, Debug, Clone, PartialEq)]
#[sqlx(type_name = "deadline_type")]
#[sqlx(rename_all = "lowercase")]
pub enum DeadlineType {
    Off,
    Soft,
    Hard,
}

#[derive(Clone, Debug)]
pub struct Tone {
    pub id: i64,
    pub name: String,
    pub user_id: i64,
    pub global: bool,
    pub stages: Json<Vec<String>>,
    pub greeting: String,
    pub unmet_behavior: GoalBehavior,
    pub deadline: DeadlineType,
}

#[derive(Clone, Debug)]
pub struct Group {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub user_id: i64,
    pub tone_id: i64,
}

#[derive(Clone, Debug)]
pub struct GroupWithInfo {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub tone_name: String,
    pub tone_stages: Json<Vec<String>>,
    pub greeting: String,
    pub unmet_behavior: GoalBehavior,
    pub deadline: DeadlineType,
    pub tone_id: i64,
    pub user_id: i64,
}

#[derive(Clone, Debug)]
pub struct GroupDisplay {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub tone_name: String,
    pub tone_stages: Vec<String>,
    pub greeting: String,
    pub unmet_behavior: GoalBehavior,
    pub deadline: DeadlineType,
    pub tone_id: i64,
    pub user_id: i64,
}

impl From<GroupWithInfo> for GroupDisplay {
    fn from(value: GroupWithInfo) -> Self {
        Self {
            id: value.id,
            title: value.title,
            description: value.description,
            tone_name: value.tone_name,
            tone_stages: value
                .tone_stages
                .iter()
                .map(|s| s.clone())
                .collect::<Vec<String>>(),
            greeting: value.greeting,
            unmet_behavior: value.unmet_behavior,
            deadline: value.deadline,
            tone_id: value.tone_id,
            user_id: value.user_id,
        }
    }
}

impl From<GroupWithInfo> for Group {
    fn from(value: GroupWithInfo) -> Self {
        Self {
            id: value.id,
            title: value.title,
            description: value.description,
            tone_id: value.tone_id,
            user_id: value.user_id,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GroupLink {
    id: i64,
    title: String,
}

#[derive(Clone, Debug)]
pub struct Goal {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub stage: i64,
    pub group_id: i64,
    pub deadline: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebauthnCredential {
    pub id: Uuid,
    pub user_id: i64,
    pub passkey: String,
}

pub async fn seed_db(pool: &SqlitePool) {
    let mut conn = pool.acquire().await.expect("to connect to database");
    let email = "rickhenry@rickhenry.dev";
    let admin_user = if let Ok(Some(u)) = sqlx::query_as!(
        User,
        r#"SELECT id, name, email, userid as "userid: Uuid", is_new_user FROM users WHERE email = $1"#,
        email
    )
    .fetch_optional(&mut conn)
    .await
    {
        u
    } else {
        let userid = Uuid::new_v4();
        let name = Some("Rick Henry".to_string());
        sqlx::query!(
            r#"INSERT INTO users(name, email, userid)
            VALUES ($1, $2, $3);"#,
            name,
            email,
            userid,
        )
        .execute(&mut conn)
        .await
        .expect("user to be created");

        sqlx::query_as!(
            User,
            r#"SELECT id, name, email, userid as "userid: Uuid", is_new_user FROM users WHERE userid = $1"#,
            userid
        )
        .fetch_one(&mut conn)
        .await
        .expect("new user to exist")
    };

    struct GlobalTone {
        name: String,
        stages: Value,
        greeting: String,
        unmet_behavior: GoalBehavior,
        deadline: DeadlineType,
    }

    let global_tones = vec![
        GlobalTone {
            name: "Gentle".into(),
            stages: json!([
                "Idea".to_string(),
                "Getting Going".to_string(),
                "Almost there!".to_string(),
                "Yayyyy".to_string(),
            ]),
            greeting: "Welcome back!! Good job checking in today!".into(),
            unmet_behavior: GoalBehavior::Hide,
            deadline: DeadlineType::Off,
        },
        GlobalTone {
            name: "Business (silly)".into(),
            stages: json!([
                "Brainstorming".to_string(),
                "\"Almost Done\"".to_string(),
                "Actually Almost Done".to_string(),
                "Eh good enough".to_string(),
            ]),
            greeting: "Get ready to synergize your goals in order to up-level your growth".into(),
            unmet_behavior: GoalBehavior::Nice,
            deadline: DeadlineType::Soft,
        },
        GlobalTone {
            name: "Serious".into(),
            stages: json!([
                "Queued".to_string(),
                "In Progress".to_string(),
                "Finishing Touches".to_string(),
                "Completed".to_string(),
            ]),
            greeting: "Welcome to your goal tracker".into(),
            unmet_behavior: GoalBehavior::Nice,
            deadline: DeadlineType::Hard,
        },
        GlobalTone {
            name: "Snarky".into(),
            stages: json!([
                "You Lazy?".to_string(),
                "Woah you started".to_string(),
                "Not done yet?".to_string(),
                "Oh finally???".to_string(),
            ]),
            greeting: "Wow you actually signed in to check. Way to go/s".into(),
            unmet_behavior: GoalBehavior::Mean,
            deadline: DeadlineType::Hard,
        },
        GlobalTone {
            name: "Boring".into(),
            stages: json!([
                "stage 1".to_string(),
                "stage 2".to_string(),
                "stage 3".to_string(),
                "stage 4".to_string(),
            ]),
            greeting: "[insert greeting]".into(),
            unmet_behavior: GoalBehavior::Nice,
            deadline: DeadlineType::Soft,
        },
        GlobalTone {
            name: "Just Colors".into(),
            stages: json!([
                "red".to_string(),
                "yellow".to_string(),
                "blue".to_string(),
                "green".to_string()
            ]),
            greeting: "Rainbow!".into(),
            unmet_behavior: GoalBehavior::Nice,
            deadline: DeadlineType::Soft,
        },
    ];

    let gentle = "Gentle".to_string();
    if sqlx::query!("SELECT id FROM tones WHERE name = $1;", gentle)
        .fetch_optional(&mut conn)
        .await
        .expect("to connect to database")
        .is_none()
    {
        let mut query_builder: QueryBuilder<Sqlite> = QueryBuilder::new(
            "INSERT INTO tones(name, user_id, global, stages, greeting, deadline, unmet_behavior)",
        );

        query_builder.push_values(global_tones.into_iter(), |mut b, tone| {
            b.push_bind(tone.name)
                .push_bind(admin_user.id)
                .push_bind(true)
                .push_bind(tone.stages)
                .push_bind(tone.greeting)
                .push_bind(tone.deadline)
                .push_bind(tone.unmet_behavior);
        });

        query_builder
            .build()
            .execute(&mut conn)
            .await
            .expect("to create global tones");
    }
}

pub fn handle_unauthorized<B>(
    mut res: dev::ServiceResponse<B>,
) -> Result<ErrorHandlerResponse<B>, actix_web::Error> {
    let redirect_to = "/login";
    *res.response_mut().status_mut() = StatusCode::SEE_OTHER;
    res.response_mut().headers_mut().insert(
        header::LOCATION,
        header::HeaderValue::from_str(redirect_to.as_ref())?,
    );
    res.response_mut().headers_mut().insert(
        header::HeaderName::from_str("HX-Redirect").map_err(ErrorInternalServerError)?,
        header::HeaderValue::from_str("true")?,
    );
    res.response_mut().headers_mut().insert(
        header::HeaderName::from_str("HX-Location").map_err(ErrorInternalServerError)?,
        header::HeaderValue::from_str(r#"{"path": "/login", "target": "html"}"#)?,
    );
    res.response_mut().headers_mut().insert(
        header::HeaderName::from_str("HX-Refresh").map_err(ErrorInternalServerError)?,
        header::HeaderValue::from_str("true")?,
    );
    res.response_mut().headers_mut().insert(
        header::HeaderName::from_str("HX-Push-Url").map_err(ErrorInternalServerError)?,
        header::HeaderValue::from_str("/location")?,
    );
    Ok(ErrorHandlerResponse::Response(res.map_into_left_body()))
}
