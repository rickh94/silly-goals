pub mod csrf_token;
pub mod mail;
pub mod queries;
pub mod routes;
pub mod session_values;

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
use sqlx::{
    types::{chrono, Uuid},
    PgPool, Postgres, QueryBuilder,
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
}

#[derive(sqlx::Type, Debug, Clone)]
#[sqlx(type_name = "goal_behavior")]
#[sqlx(rename_all = "lowercase")]
pub enum GoalBehavior {
    Hide,
    Nice,
    Mean,
}

#[derive(sqlx::Type, Debug, Clone)]
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
    pub stages: Vec<String>,
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
pub struct GroupDisplay {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub tone_name: String,
    pub tone_stages: Vec<String>,
    pub greeting: String,
    pub unmet_behavior: GoalBehavior,
    pub deadline: DeadlineType,
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
    pub stage: i16,
    pub group_id: i64,
    pub deadline: Option<chrono::NaiveDate>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WebauthnCredential {
    pub id: Uuid,
    pub user_id: i64,
    pub passkey: String,
}

pub async fn seed_db(pool: &PgPool) {
    let mut conn = pool.acquire().await.expect("to connect to database");
    let admin_user = if let Ok(Some(u)) = sqlx::query_as!(
        User,
        "SELECT id, name, email, userid FROM users WHERE email = $1",
        "rickhenry@rickhenry.dev".into()
    )
    .fetch_optional(&mut conn)
    .await
    {
        u
    } else {
        sqlx::query_as!(
            User,
            "INSERT INTO users(name, email) VALUES ($1, $2) RETURNING *;",
            "Rick Henry".into(),
            "rickhenry@rickhenry.dev".into(),
        )
        .fetch_one(&mut conn)
        .await
        .expect("user to be created")
    };

    struct GlobalTone {
        name: String,
        stages: Vec<String>,
        greeting: String,
        unmet_behavior: GoalBehavior,
        deadline: DeadlineType,
    }

    let global_tones = vec![
        GlobalTone {
            name: "Gentle".into(),
            stages: vec![
                "Idea".into(),
                "Getting Going".into(),
                "Almost there!".into(),
                "Yayyyy".into(),
            ],
            greeting: "Welcome back!! Good job checking in today!".into(),
            unmet_behavior: GoalBehavior::Hide,
            deadline: DeadlineType::Off,
        },
        GlobalTone {
            name: "Business (silly)".into(),
            stages: vec![
                "Brainstorming".into(),
                "\"Almost Done\"".into(),
                "Actually Almost Done".into(),
                "Eh good enough".into(),
            ],
            greeting: "Get ready to synergize your goals in order to up-level your growth".into(),
            unmet_behavior: GoalBehavior::Nice,
            deadline: DeadlineType::Soft,
        },
        GlobalTone {
            name: "Serious".into(),
            stages: vec![
                "Queued".into(),
                "In Progress".into(),
                "Finishing Touches".into(),
                "Completed".into(),
            ],
            greeting: "Welcome to your goal tracker".into(),
            unmet_behavior: GoalBehavior::Nice,
            deadline: DeadlineType::Hard,
        },
        GlobalTone {
            name: "Snarky".into(),
            stages: vec![
                "You Lazy?".into(),
                "Woah you started".into(),
                "Not done yet?".into(),
                "Oh finally???".into(),
            ],
            greeting: "Wow you actually signed in to check. Way to go/s".into(),
            unmet_behavior: GoalBehavior::Mean,
            deadline: DeadlineType::Hard,
        },
        GlobalTone {
            name: "Boring".into(),
            stages: vec![
                "stage 1".into(),
                "stage 2".into(),
                "stage 3".into(),
                "stage 4".into(),
            ],
            greeting: "[insert greeting]".into(),
            unmet_behavior: GoalBehavior::Nice,
            deadline: DeadlineType::Soft,
        },
        GlobalTone {
            name: "Just Colors".into(),
            stages: vec!["red".into(), "yellow".into(), "blue".into(), "green".into()],
            greeting: "Rainbow!".into(),
            unmet_behavior: GoalBehavior::Nice,
            deadline: DeadlineType::Soft,
        },
    ];

    if sqlx::query!("SELECT id FROM tones WHERE name = $1;", "Gentle".into())
        .fetch_optional(&mut conn)
        .await
        .expect("to connect to database")
        .is_none()
    {
        let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
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
    // TODO: delete session to prevent redirect loops or check for valid user
    // on login page
    let redirect_to = "/login";
    *res.response_mut().status_mut() = StatusCode::SEE_OTHER;
    res.response_mut().headers_mut().insert(
        header::LOCATION,
        header::HeaderValue::from_str(redirect_to.as_ref())?,
    );
    Ok(ErrorHandlerResponse::Response(res.map_into_left_body()))
}
