use actix_identity::Identity;
use actix_web::error::{ErrorInternalServerError, ErrorNotFound, ErrorUnauthorized};
use anyhow::anyhow;
use log::error;
use sqlx::{
    pool::PoolConnection,
    types::{Json, Uuid},
    Sqlite,
};

use crate::{DeadlineType, Goal, GoalBehavior, GroupLink, GroupWithInfo, User};

pub async fn get_user_from_identity(
    conn: &mut PoolConnection<Sqlite>,
    identity: &Identity,
) -> actix_web::Result<User> {
    let userid = identity.id().map_err(ErrorInternalServerError)?;
    let user_uuid = Uuid::parse_str(&userid).map_err(ErrorInternalServerError)?;
    sqlx::query_as!(
        User,
        r#"SELECT id, name, userid as "userid: Uuid", email FROM users
            WHERE userid = $1"#,
        user_uuid
    )
    .fetch_one(conn)
    .await
    .map_err(|err| match err {
        sqlx::error::Error::RowNotFound => ErrorUnauthorized(err),
        err => {
            error!("Error communicating with database: {}", err);
            ErrorInternalServerError(err)
        }
    })
}

pub async fn get_user_by_email(
    conn: &mut PoolConnection<Sqlite>,
    email: &str,
) -> actix_web::Result<User> {
    let email = email.to_lowercase();
    sqlx::query_as!(
        User,
        r#"SELECT id, email, name, userid as "userid: Uuid" FROM users WHERE email = $1"#,
        email,
    )
    .fetch_one(conn)
    .await
    .map_err(|err| {
        error!("Error communicating with database: {}", err);
        ErrorInternalServerError(err)
    })
}

/// Check whether a user exists. Returns Ok(()) if the use exists, anyhow error
/// if not
pub async fn check_for_user_from_identity(
    conn: &mut PoolConnection<Sqlite>,
    identity: &Option<Identity>,
) -> anyhow::Result<()> {
    if identity.is_none() {
        return Err(anyhow!("No identity"));
    }
    // just checked for none
    #[allow(clippy::unwrap_used)]
    let identity = identity.as_ref().unwrap();

    let userid = identity
        .id()
        .map_err(|_| anyhow!("Could not get user id"))?;
    let user_uuid = Uuid::parse_str(&userid).map_err(|_| anyhow!("invalid userid"))?;
    let id = sqlx::query_scalar!("SELECT id FROM users WHERE userid = $1", user_uuid)
        .fetch_optional(conn)
        .await
        .map_err(|_| anyhow!("Could not get matching user"))?;

    if id.is_some() {
        Ok(())
    } else {
        Err(anyhow!("No matching user"))
    }
}

pub async fn get_group_with_info(
    conn: &mut PoolConnection<Sqlite>,
    user_id: i64,
    group_id: i64,
) -> actix_web::Result<GroupWithInfo> {
    sqlx::query_as!(
        GroupWithInfo,
        r#"SELECT 
        g.id,
        g.title, 
        g.description, 
        t.name as tone_name, 
        t.stages as "tone_stages: Json<Vec<String>>", 
        t.greeting, 
        t.unmet_behavior as "unmet_behavior: GoalBehavior", 
        t.deadline as "deadline: DeadlineType"
        FROM groups g
        LEFT JOIN tones t
        ON g.tone_id = t.id
        WHERE g.user_id = $1 AND g.id = $2;"#,
        user_id,
        group_id
    )
    .fetch_one(conn)
    .await
    .map_err(|err| match err {
        sqlx::Error::RowNotFound => ErrorNotFound(err),
        e => ErrorInternalServerError(e),
    })
}

pub async fn get_group_links(
    conn: &mut PoolConnection<Sqlite>,
    user_id: i64,
) -> actix_web::Result<Vec<GroupLink>> {
    sqlx::query_as!(
        GroupLink,
        "SELECT id, title FROM groups WHERE user_id = $1",
        user_id
    )
    .fetch_all(conn)
    .await
    .map_err(ErrorInternalServerError)
}

pub async fn get_goals_for_group(
    conn: &mut PoolConnection<Sqlite>,
    group_id: i64,
) -> actix_web::Result<Vec<Goal>> {
    sqlx::query_as!(Goal, "SELECT * FROM goals WHERE group_id = $1;", group_id)
        .fetch_all(conn)
        .await
        .map_err(ErrorInternalServerError)
}
