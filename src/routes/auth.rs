use crate::{csrf_token::CsrfToken, mail::*, session_values::*, SessionValue, User};
use actix_identity::Identity;
use actix_session::Session;
use actix_web::{
    error::{ErrorBadRequest, ErrorInternalServerError, ErrorUnauthorized},
    web::{self, Form},
    *,
};
use anyhow::anyhow;
use askama::Template;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use nanoid::nanoid;
use serde::Deserialize;
use shuttle_runtime::tracing::{error, info};
use sqlx::PgPool;

#[derive(Template)]
#[template(path = "register.html")]
struct RegisterStart {
    title: String,
    csrf_token: CsrfToken,
}

/// Start Registration for the user account
#[get("register")]
async fn register(session: Session, identity: Option<Identity>) -> Result<HttpResponse> {
    if identity.is_some() {
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/profile"))
            .finish());
    }
    let csrf_token = CsrfToken::get_or_create(&session).map_err(|err| {
        info!("CSRF error: {}", err);
        ErrorInternalServerError(err)
    })?;
    let body = RegisterStart {
        title: "Register . Silly Goals".into(),
        csrf_token,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

#[derive(Template)]
#[template(path = "register_finish.html")]
struct RegisterFinish {
    title: String,
    csrf_token: CsrfToken,
    error: Option<String>,
}

#[derive(Deserialize)]
pub struct RegistrationForm {
    pub email: String,
    pub csrftoken: String,
}

/// Receive email from user form and send back otp code form
#[post("register")]
async fn post_register(
    session: Session,
    form: Form<RegistrationForm>,
    pool: web::Data<PgPool>,
    mailer: web::Data<AsyncSmtpTransport<Tokio1Executor>>,
) -> Result<RegisterFinish> {
    CsrfToken::verify_from_session(&session, form.csrftoken.as_str())?;
    LoginCode::remove(&session);
    LoginEmail::remove(&session);
    RegistrationEmail::remove(&session);

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(|err| ErrorInternalServerError(err))?;

    let existing_user_count = sqlx::query_scalar!(
        "SELECT COUNT(id)
            FROM users
            WHERE email
            ILIKE $1",
        form.email
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|err| {
        error!("Error communicating with database: {}", err);
        ErrorInternalServerError(err)
    })?
    .unwrap_or(0);

    if existing_user_count > 0 {
        return Err(ErrorBadRequest(anyhow!(
            "Could not register user with that email."
        )));
    }

    let login_code = LoginCode::new();
    let registration_email = RegistrationEmail::from(&form.email);

    login_code.save(&session)?;
    registration_email.save(&session)?;

    let message = build_email_for_user(
        &registration_email,
        "Registration Code for Silly Goals",
        &format!("Use code {login_code} to confirm your new account and log in."),
    )?;

    mailer.send(message).await.map_err(|err| {
        error!("Could not send registration email, {}", err);
        ErrorInternalServerError(err)
    })?;

    let csrf_token = CsrfToken::get_or_create(&session)?;

    Ok(RegisterFinish {
        title: "Register . Silly Goals".into(),
        csrf_token,
        error: None,
    })
}

#[derive(Deserialize)]
struct RegistrationCodeForm {
    pub code: String,
    pub csrftoken: String,
}

/// Complete registration by verifying the submitted code with the session and
/// creating a new user in the database
#[post("finish-registration")]
async fn finish_registration(
    req: HttpRequest,
    session: Session,
    form: Form<RegistrationCodeForm>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    CsrfToken::verify_from_session(&session, form.csrftoken.as_str())?;

    let user_registration_email = match RegistrationEmail::get(&session) {
        Ok(Some(e)) => e,
        Ok(None) => {
            return Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/register"))
                .finish())
        }
        _ => {
            return Err(ErrorInternalServerError(anyhow!(
                "Could not get email from session"
            )))
        }
    };

    let correct_login_code = match LoginCode::get(&session) {
        Ok(Some(e)) => e,
        Ok(None) => {
            return Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/register"))
                .finish())
        }
        _ => {
            return Err(ErrorInternalServerError(anyhow!(
                "Could not get code from session"
            )))
        }
    };

    if !correct_login_code.verify(&form.code) {
        let csrf_token = CsrfToken::get_or_create(&session)?;
        let body = RegisterFinish {
            csrf_token,
            title: "Register . Silly Goals".into(),
            error: Some("Invalid code".into()),
        }
        .render()
        .map_err(|err| {
            error!("Template error: {}", err);
            ErrorInternalServerError(err)
        })?;
        return Ok(HttpResponse::Ok().body(body));
    }

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(|err| ErrorInternalServerError(err))?;

    let new_user = sqlx::query_as!(
        User,
        "INSERT INTO users(email, userid)
            VALUES ($1, $2)
            RETURNING *;",
        user_registration_email.to_lowercase(),
        nanoid!()
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|err| {
        error!("Error communicating with database: {}", err);
        ErrorInternalServerError(err)
    })?;

    Identity::login(&req.extensions(), new_user.userid).map_err(|err| {
        error!("Error Logging in new user: {}", err);
        ErrorInternalServerError(err)
    })?;

    RegistrationEmail::remove(&session);
    LoginCode::remove(&session);

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/dashboard"))
        .finish())
}

#[derive(Template)]
#[template(path = "login.html")]
struct LoginStart {
    title: String,
    csrf_token: CsrfToken,
}

/// Start Login for the user account
#[get("login")]
async fn login(session: Session, identity: Option<Identity>) -> Result<HttpResponse> {
    if identity.is_some() {
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/profile"))
            .finish());
    }
    let csrf_token = CsrfToken::get_or_create(&session).map_err(|err| {
        info!("CSRF error: {}", err);
        ErrorInternalServerError(err)
    })?;

    let body = LoginStart {
        title: "Login . Silly Goals".into(),
        csrf_token,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

#[derive(Template)]
#[template(path = "login_finish.html")]
struct LoginFinish {
    title: String,
    csrf_token: CsrfToken,
    error: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub csrftoken: String,
}

/// Receive email from user form and send back otp code form
#[post("login")]
async fn post_login(
    session: Session,
    form: Form<LoginForm>,
    pool: web::Data<PgPool>,
    mailer: web::Data<AsyncSmtpTransport<Tokio1Executor>>,
) -> Result<HttpResponse> {
    CsrfToken::verify_from_session(&session, form.csrftoken.as_str())?;
    LoginCode::remove(&session);
    LoginEmail::remove(&session);
    RegistrationEmail::remove(&session);

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(|err| ErrorInternalServerError(err))?;

    let user = sqlx::query_as!(
        User,
        "SELECT id, userid, name, email
            FROM users
            WHERE email = $1",
        form.email.to_lowercase()
    )
    .fetch_optional(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    let csrf_token = CsrfToken::get_or_create(&session)?;

    let body = LoginFinish {
        title: "Login . Silly Goals".into(),
        csrf_token,
        error: None,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    if user.is_none() {
        return Ok(HttpResponse::Ok().body(body));
    }

    // We have already checked for none, so unwrap is ok here.
    #[allow(clippy::unwrap_used)]
    let user = user.unwrap();

    let login_code = LoginCode::new();
    let login_email = LoginEmail::from(&form.email);

    login_code.save(&session)?;
    login_email.save(&session)?;

    let message = build_email_for_user(
        &user.email,
        "Login Code for Silly Goals",
        &format!("Use code {login_code} to log in to your account."),
    )?;

    mailer.send(message).await.map_err(|err| {
        error!("Could not send login email, {}", err);
        ErrorInternalServerError(err)
    })?;

    Ok(HttpResponse::Ok().body(body))
}

#[derive(Deserialize)]
struct LoginCodeForm {
    pub code: String,
    pub csrftoken: String,
}

/// Complete login by verifying the submitted code with the session and logging
/// in the user
#[post("finish-login")]
async fn finish_login(
    req: HttpRequest,
    session: Session,
    form: Form<LoginCodeForm>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    CsrfToken::verify_from_session(&session, form.csrftoken.as_str())?;

    let user_login_email = match LoginEmail::get(&session) {
        Ok(Some(e)) => e,
        Ok(None) => {
            return Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/login"))
                .finish())
        }
        _ => {
            return Err(ErrorInternalServerError(anyhow!(
                "Could not get email from session"
            )))
        }
    };

    let correct_login_code = match LoginCode::get(&session) {
        Ok(Some(e)) => e,
        Ok(None) => {
            return Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/login"))
                .finish())
        }
        _ => {
            return Err(ErrorInternalServerError(anyhow!(
                "Could not get code from session"
            )))
        }
    };

    if !correct_login_code.verify(&form.code) {
        let csrf_token = CsrfToken::get_or_create(&session)?;
        let body = LoginFinish {
            csrf_token,
            title: "Login . Silly Goals".into(),
            error: Some("Invalid code".into()),
        }
        .render()
        .map_err(|err| {
            error!("Template error: {}", err);
            ErrorInternalServerError(err)
        })?;
        return Ok(HttpResponse::Ok().body(body));
    }

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(|err| ErrorInternalServerError(err))?;

    let user = sqlx::query_as!(
        User,
        "SELECT id, email, name, userid FROM users WHERE email = $1",
        user_login_email.to_lowercase(),
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|err| {
        error!("Error communicating with database: {}", err);
        ErrorInternalServerError(err)
    })?;

    Identity::login(&req.extensions(), user.userid).map_err(|err| {
        error!("Error Logging in new user: {}", err);
        ErrorInternalServerError(err)
    })?;

    LoginEmail::remove(&session);
    LoginCode::remove(&session);

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/dashboard"))
        .finish())
}

#[get("logout")]
async fn logout(identity: Identity) -> HttpResponse {
    identity.logout();
    HttpResponse::SeeOther()
        .insert_header(("Location", "/"))
        .finish()
}

// TODO: webauthn registration

#[derive(Template)]
#[template(path = "profile.html")]
struct Profile {
    title: String,
    user: User,
}

/// Display user profie information
#[get("/profile")]
async fn profile(identity: Identity, pool: web::Data<PgPool>) -> actix_web::Result<Profile> {
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;
    let user_id = identity.id().map_err(ErrorInternalServerError)?;
    let user = sqlx::query_as!(
        User,
        "SELECT id, name, userid, email FROM users
            WHERE userid = $1",
        user_id,
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|err| match err {
        sqlx::error::Error::RowNotFound => ErrorUnauthorized(err),
        err => {
            error!("Error communicating with database: {}", err);
            ErrorInternalServerError(err)
        }
    })?;

    Ok(Profile {
        title: "Profile . Silly Goals".into(),
        user,
    })
}
