use actix_identity::Identity;
use actix_web::error::{ErrorInternalServerError, ErrorUnauthorized};
use log::error;
use sqlx::{pool::PoolConnection, types::Uuid, Postgres};

use crate::User;

pub async fn get_user_from_identity(
    conn: &mut PoolConnection<Postgres>,
    identity: &Identity,
) -> actix_web::Result<User> {
    let user_id = identity.id().map_err(ErrorInternalServerError)?;
    sqlx::query_as!(
        User,
        "SELECT id, name, userid, email FROM users
            WHERE userid = $1",
        Uuid::parse_str(&user_id).map_err(ErrorInternalServerError)?
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
    conn: &mut PoolConnection<Postgres>,
    email: &str,
) -> actix_web::Result<User> {
    sqlx::query_as!(
        User,
        "SELECT id, email, name, userid FROM users WHERE email = $1",
        email.to_lowercase(),
    )
    .fetch_one(conn)
    .await
    .map_err(|err| {
        error!("Error communicating with database: {}", err);
        ErrorInternalServerError(err)
    })
}
