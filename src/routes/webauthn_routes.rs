use actix_identity::Identity;
use actix_session::Session;
use actix_web::{
    error::{ErrorBadRequest, ErrorInternalServerError},
    get, post,
    web::{self, Json},
    HttpMessage, HttpRequest, HttpResponse,
};
use log::{debug, error};
use serde::{Deserialize, Serialize};
use sqlx::{types::Uuid, SqlitePool};
use webauthn_rs::prelude::*;

use crate::{queries, session_values::LoginEmail, SessionValue, WebauthnCredential};

#[get("/webauthn/register")]
async fn start_registration(
    identity: Identity,
    session: Session,
    webauthn: web::Data<Webauthn>,
    pool: web::Data<SqlitePool>,
) -> actix_web::Result<HttpResponse> {
    PasskeyRegistration::remove(&session);

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let allow_credential_items = sqlx::query_as!(
        WebauthnCredential,
        r#"SELECT id as "id: Uuid", user_id, passkey
        FROM webauthn_credentials
        WHERE user_id = $1"#,
        user.id,
    )
    .fetch_all(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    let excluded_credentials = allow_credential_items
        .iter()
        .filter_map(|i| serde_json::from_str::<Passkey>(&i.passkey).ok())
        .map(|k| k.cred_id().clone())
        .collect();

    let (ccr, reg_state) = webauthn
        .start_passkey_registration(
            user.userid,
            &user.email,
            &user.email,
            Some(excluded_credentials),
        )
        .map_err(|err| {
            debug!("challenge register -> {:?}", err);
            ErrorInternalServerError(err)
        })?;

    reg_state.save(&session)?;

    Ok(HttpResponse::Ok().json(&ccr))
}

#[post("/webauthn/register")]
async fn finish_registration(
    reg: Json<RegisterPublicKeyCredential>,
    identity: Identity,
    session: Session,
    webauthn: web::Data<Webauthn>,
    pool: web::Data<SqlitePool>,
) -> actix_web::Result<HttpResponse> {
    let reg_state = PasskeyRegistration::get(&session)
        .map_err(ErrorInternalServerError)?
        .ok_or(ErrorInternalServerError("Could not get registration state"))?;
    let sk = webauthn
        .finish_passkey_registration(&reg, &reg_state)
        .map_err(|err| {
            error!("challenge register -> {:?}", err);
            ErrorInternalServerError(err)
        })?;

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let sk_json = serde_json::to_string(&sk).map_err(|err| {
        error!("Could not save passkey, {}", err);
        ErrorInternalServerError(err)
    })?;

    let credential_id = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO webauthn_credentials(id, user_id, passkey)
        VALUES ($1, $2, $3);",
        credential_id,
        user.id,
        sk_json,
    )
    .execute(&mut conn)
    .await
    .map_err(|err| {
        error!("Error communicating with database: {}", err);
        ErrorInternalServerError(err)
    })?;
    PasskeyRegistration::remove(&session);
    Ok(HttpResponse::Ok().finish())
}

#[derive(Clone, Deserialize, Serialize)]
struct AuthState {
    userid: Uuid,
    passkey_auth: PasskeyAuthentication,
}

impl SessionValue for AuthState {
    fn save_name() -> &'static str {
        "auth_state"
    }
}

#[get("/webauthn/login")]
async fn start_login(
    pool: web::Data<SqlitePool>,
    session: Session,
    webauthn: web::Data<Webauthn>,
) -> actix_web::Result<HttpResponse> {
    let login_email = LoginEmail::get(&session).map_err(ErrorInternalServerError)?;
    if login_email.is_none() {
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/login"))
            .finish());
    }

    // checked for none above so unwrapping is fine
    #[allow(clippy::unwrap_used)]
    let login_email = login_email.unwrap();

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_by_email(&mut conn, &login_email).await?;

    let allow_credential_items = sqlx::query_as!(
        WebauthnCredential,
        r#"SELECT id as "id: Uuid", user_id, passkey 
        FROM webauthn_credentials 
        WHERE user_id = $1;"#,
        user.id,
    )
    .fetch_all(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;
    let allowed_credentials: Vec<Passkey> = allow_credential_items
        .iter()
        .filter_map(|i| serde_json::from_str(&i.passkey).ok())
        .collect();

    let (rcr, passkey_auth) = webauthn
        .start_passkey_authentication(&allowed_credentials)
        .map_err(|err| {
            error!("challenge authenticate {:?}", err);
            ErrorInternalServerError(err)
        })?;

    LoginEmail::remove(&session);

    AuthState::remove(&session);
    let auth_state = AuthState {
        userid: user.userid,
        passkey_auth,
    };

    auth_state.save(&session)?;

    Ok(HttpResponse::Ok().json(&rcr))
}

#[post("/webauthn/login")]
async fn finish_login(
    req: HttpRequest,
    auth: Json<PublicKeyCredential>,
    session: Session,
    webauthn: web::Data<Webauthn>,
) -> actix_web::Result<HttpResponse> {
    let auth_state = AuthState::get(&session).map_err(ErrorInternalServerError)?;

    if auth_state.is_none() {
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/login"))
            .finish());
    }

    // checked for none above so unwrapping is fine
    #[allow(clippy::unwrap_used)]
    let auth_state = auth_state.unwrap();

    let _auth_result = webauthn
        .finish_passkey_authentication(&auth, &auth_state.passkey_auth)
        .map_err(ErrorBadRequest)?;

    Identity::login(&req.extensions(), auth_state.userid.to_string()).map_err(|err| {
        error!("Could not login user, Error: {}", err);
        ErrorInternalServerError(err)
    })?;

    AuthState::remove(&session);
    Ok(HttpResponse::Ok().finish())
}
