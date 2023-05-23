use actix_identity::Identity;
use actix_web::error::{ErrorInternalServerError, ErrorUnauthorized};
use log::error;
use sqlx::{pool::PoolConnection, types::Uuid, Sqlite};

use crate::User;

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
