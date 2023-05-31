use crate::{
    csrf_token::CsrfToken,
    htmx::{self, IsHtmx},
    mail::*,
    queries,
    session_values::*,
    templates::*,
    SessionValue, User,
};
use actix_identity::Identity;
use actix_session::Session;
use actix_web::{
    error::ErrorInternalServerError,
    web::{self, Form},
    *,
};
use anyhow::anyhow;
use askama::Template;
use lettre::{AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use log::{error, info};
use serde::Deserialize;
use sqlx::{pool::PoolConnection, types::Uuid, Sqlite, SqlitePool};

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
    pool: web::Data<SqlitePool>,
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
            WHERE LOWER(email)
            LIKE LOWER($1)",
        form.email
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|err| {
        error!("Error communicating with database: {}", err);
        ErrorInternalServerError(err)
    })?;

    let csrf_token = CsrfToken::get_or_create(&session)?;

    let message = if existing_user_count > 0 {
        build_email_for_user(
            &form.email,
            "Silly Goals Registration",
            "Someone tried to register a new Silly Goals account with \
            this email. If this was you, Good News! you're already \
            registered, and you can just login instead. If not, that's a \
            little weird, but we stopped them. You might want to check for \
            weird activity on your email",
        )?
    } else {
        let login_code = LoginCode::new();
        let registration_email = RegistrationEmail::from(&form.email);

        login_code.save(&session)?;
        registration_email.save(&session)?;

        build_email_for_user(
            &registration_email,
            "Registration Code for Silly Goals",
            &format!("Use code {login_code} to confirm your new account and log in."),
        )?
    };

    tokio::spawn(async move {
        match mailer.send(message).await {
            Ok(_) => (),
            Err(e) => {
                error!("Failed to send message: {}", e);
            }
        }
    });

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
    pool: web::Data<SqlitePool>,
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

    let email = user_registration_email.to_lowercase();
    let userid = Uuid::new_v4();
    sqlx::query!(
        r#"INSERT INTO users(email, userid)
            VALUES ($1, $2);"#,
        email,
        userid
    )
    .execute(&mut conn)
    .await
    .map_err(|err| {
        error!("Error communicating with database: {}", err);
        ErrorInternalServerError(err)
    })?;

    let new_user = sqlx::query_as!(
        User,
        r#"SELECT id, userid as "userid: Uuid", email, name, is_new_user
        FROM users 
        WHERE userid = $1;"#,
        userid
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|err| {
        error!("Error communicating with database: {}", err);
        ErrorInternalServerError(err)
    })?;

    Identity::login(&req.extensions(), new_user.userid.to_string()).map_err(|err| {
        error!("Error Logging in new user: {}", err);
        ErrorInternalServerError(err)
    })?;

    RegistrationEmail::remove(&session);
    LoginCode::remove(&session);

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/dashboard"))
        .finish())
}

/// Start Login for the user account
#[get("login")]
async fn login(
    session: Session,
    identity: Option<Identity>,
    pool: web::Data<SqlitePool>,
) -> Result<HttpResponse> {
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;
    if identity.is_some()
        && queries::check_for_user_from_identity(&mut conn, &identity)
            .await
            .is_ok()
    {
        return Ok(HttpResponse::SeeOther()
            .insert_header(("Location", "/profile"))
            .finish());
    } else {
        // In case an old session is hanging around with an invalid logged in user,
        // we delete the session, then reload to get a new one (could be nicer i guess)
        if let Some(identity) = identity {
            identity.logout();
            return Ok(HttpResponse::SeeOther()
                .insert_header(("Location", "/login"))
                .finish());
        }
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

#[derive(Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub csrftoken: String,
}

/// Receive email from user form and send back login selection
#[post("login")]
async fn post_login(
    session: Session,
    form: Form<LoginForm>,
    pool: web::Data<SqlitePool>,
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
        r#"SELECT id, userid as "userid: Uuid", name, email, is_new_user
            FROM users
            WHERE email = Lower($1)"#,
        form.email
    )
    .fetch_optional(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    if let Some(user) = user {
        let login_email = LoginEmail::from(&user.email);
        login_email.save(&session)?;
    } else {
        let message = build_email_for_user(
            &form.email,
            "Login Attempt at Silly Goals",
            "Someone tried to use your email to login at Silly Goals. \
            If this was you, you'll need to register first. Otherwise you \
            might want to look for other weird activity on your email. \
            They were not able to log in.",
        )?;

        tokio::spawn(async move {
            match mailer.send(message).await {
                Ok(_) => (),
                Err(e) => {
                    error!("Could not send failed login message: {}", e);
                }
            }
        });
    }

    let body = LoginSelect {
        title: "Login . Silly Goals".into(),
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

#[get("/login-code")]
async fn login_with_code(
    session: Session,
    mailer: web::Data<AsyncSmtpTransport<Tokio1Executor>>,
) -> actix_web::Result<HttpResponse> {
    let login_email = LoginEmail::get(&session).map_err(ErrorInternalServerError)?;

    if let Some(login_email) = login_email {
        let login_code = LoginCode::new();
        login_code.save(&session)?;

        let message = build_email_for_user(
            &login_email,
            "Login Code for Silly Goals",
            &format!("Use code {login_code} to log in to your account."),
        )?;

        tokio::spawn(async move {
            match mailer.send(message).await {
                Ok(_) => (),
                Err(e) => {
                    error!("Could not sent message: {}", e);
                }
            }
        });
    }

    let csrf_token = CsrfToken::get_or_create(&session)?;

    let body = LoginFinish {
        title: "Login . Silly Goals".into(),
        csrf_token,
        error: None,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

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
    pool: web::Data<SqlitePool>,
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

    let user = queries::get_user_by_email(&mut conn, &user_login_email).await?;

    Identity::login(&req.extensions(), user.userid.to_string()).map_err(|err| {
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

/// Display user profie information
#[get("/profile")]
async fn profile(
    identity: Identity,
    pool: web::Data<SqlitePool>,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;
    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let groups = queries::get_group_links(&mut conn, user.id).await?;

    if *is_hx {
        let body = ProfilePartial { user }
            .render()
            .map_err(ErrorInternalServerError)?;
        return Ok(HttpResponse::Ok()
            .insert_header(("HX-Trigger-After-Swap", "updateLocation"))
            .body(body));
    }

    let body = ProfilePage {
        title: "Silly Goals".into(),
        user,
        groups,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

/// Delete the user's profile
#[post("/profile/delete")]
async fn delete_profile(
    identity: Identity,
    pool: web::Data<SqlitePool>,
) -> actix_web::Result<HttpResponse> {
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    sqlx::query!("DELETE FROM users WHERE id = $1", user.id)
        .execute(&mut conn)
        .await
        .map_err(ErrorInternalServerError)?;

    identity.logout();

    Ok(HttpResponse::Ok().finish())
}

/// Edit user's name
#[get("/profile/edit/name")]
async fn profile_edit_name(
    identity: Identity,
    session: Session,
    pool: web::Data<SqlitePool>,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;
    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let csrf_token = CsrfToken::get_or_create(&session)?;

    if *is_hx {
        let body = ProfileEditNamePartial { user, csrf_token }
            .render()
            .map_err(ErrorInternalServerError)?;
        return Ok(HttpResponse::Ok()
            .insert_header(("HX-Trigger-After-Swap", "updateLocation"))
            .body(body));
    }

    let groups = queries::get_group_links(&mut conn, user.id).await?;

    let body = ProfileEditNamePage {
        title: "Silly Goals".into(),
        user,
        groups,
        csrf_token,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

#[derive(Deserialize)]
struct UserNameForm {
    name: String,
    csrftoken: String,
}

#[post("/profile/edit/name")]
async fn post_profile_edit_name(
    identity: Identity,
    session: Session,
    pool: web::Data<SqlitePool>,
    form: web::Form<UserNameForm>,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    CsrfToken::verify_from_session(&session, &form.csrftoken)?;

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let userid = identity.id().map_err(ErrorInternalServerError)?;
    let user_uuid = Uuid::parse_str(&userid).map_err(ErrorInternalServerError)?;

    sqlx::query!(
        "UPDATE users SET name = $1 WHERE userid = $2;",
        form.name,
        user_uuid
    )
    .execute(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    if *is_hx {
        let body = ProfilePartial { user }
            .render()
            .map_err(ErrorInternalServerError)?;
        let notification = htmx::hx_trigger_notification(
            "Name Updated".into(),
            format!("Your name has been changed to {}", form.name),
            htmx::NotificationVariant::Success,
            true,
        );
        return Ok(HttpResponse::Ok()
            .append_header(notification)
            .append_header(("HX-Trigger", "updateLocation"))
            .body(body));
    }

    let groups = queries::get_group_links(&mut conn, user.id).await?;

    let body = ProfilePage {
        title: "Silly Goals".into(),
        user,
        groups,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

/// Edit user's email
#[get("/profile/edit/email")]
async fn profile_edit_email(
    identity: Identity,
    session: Session,
    pool: web::Data<SqlitePool>,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    LoginCode::remove(&session);
    ChangeEmail::remove(&session);
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;
    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let csrf_token = CsrfToken::get_or_create(&session)?;

    if *is_hx {
        let body = ProfileEditEmailPartial {
            user,
            csrf_token,
            error: None,
        }
        .render()
        .map_err(ErrorInternalServerError)?;
        return Ok(HttpResponse::Ok()
            .insert_header(("HX-Trigger-After-Swap", "updateLocation"))
            .body(body));
    }

    let groups = queries::get_group_links(&mut conn, user.id).await?;

    let body = ProfileEditEmailPage {
        title: "Silly Goals".into(),
        user,
        groups,
        csrf_token,
        error: None,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

#[derive(Deserialize)]
struct UserEmailForm {
    email: String,
    csrftoken: String,
}

#[post("/profile/edit/email")]
async fn post_profile_edit_email(
    identity: Identity,
    session: Session,
    pool: web::Data<SqlitePool>,
    form: web::Form<UserEmailForm>,
    mailer: web::Data<AsyncSmtpTransport<Tokio1Executor>>,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    CsrfToken::verify_from_session(&session, &form.csrftoken)?;
    LoginCode::remove(&session);
    ChangeEmail::remove(&session);

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;
    let csrf_token = CsrfToken::get_or_create(&session)?;

    let email_exists = sqlx::query_scalar!(
        "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1);",
        form.email
    )
    .fetch_one(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    if email_exists != 0 {
        let body = if *is_hx {
            ProfileEditEmailPartial {
                user,
                csrf_token,
                error: Some("Email is not available".into()),
            }
            .render()
            .map_err(ErrorInternalServerError)?
        } else {
            let groups = queries::get_group_links(&mut conn, user.id).await?;
            ProfileEditEmailPage {
                title: "Silly Goals".into(),
                user,
                csrf_token,
                error: Some("Email is not available".into()),
                groups,
            }
            .render()
            .map_err(ErrorInternalServerError)?
        };
        return Ok(HttpResponse::Ok().body(body));
    }

    let change_email = ChangeEmail::from(&form.email);
    change_email.save(&session)?;

    let login_code = LoginCode::new();
    login_code.save(&session)?;

    let message = build_email_for_user(
        &form.email,
        "Confirmation Code for Silly Goals",
        &format!("Use code {login_code} to confirm your email address."),
    )?;

    tokio::spawn(async move {
        match mailer.send(message).await {
            Ok(_) => (),
            Err(e) => {
                error!("Could not sent message: {}", e);
            }
        }
    });

    if *is_hx {
        let body = ProfileConfirmEmailPartial {
            csrf_token,
            error: None,
        }
        .render()
        .map_err(ErrorInternalServerError)?;
        return Ok(HttpResponse::Ok().body(body));
    }

    let groups = queries::get_group_links(&mut conn, user.id).await?;

    let body = ProfileConfirmEmailPage {
        title: "Silly Goals".into(),
        user,
        groups,
        csrf_token,
        error: None,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

#[derive(Deserialize)]
struct ConfirmEmailForm {
    code: String,
    csrftoken: String,
}

async fn confirm_email_invalid_session_response(
    conn: &mut PoolConnection<Sqlite>,
    identity: &Identity,
    session: &Session,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    let user = queries::get_user_from_identity(conn, &identity).await?;
    let csrf_token = CsrfToken::get_or_create(&session)?;
    let body = if *is_hx {
        ProfileEditEmailPartial {
            user,
            csrf_token,
            error: Some("Your code has expired, please try again.".into()),
        }
        .render()
        .map_err(ErrorInternalServerError)?
    } else {
        let groups = queries::get_group_links(conn, user.id).await?;
        ProfileEditEmailPage {
            title: "Silly Goals".into(),
            user,
            groups,
            csrf_token,
            error: Some("Your code has expired, please try again.".into()),
        }
        .render()
        .map_err(ErrorInternalServerError)?
    };
    Ok(HttpResponse::Ok()
        .insert_header(("HX-Trigger-After-Swap", "updateLocation"))
        .body(body))
}

#[post("/profile/edit/email/confirm")]
async fn post_profile_confirm_email(
    identity: Identity,
    session: Session,
    pool: web::Data<SqlitePool>,
    form: web::Form<ConfirmEmailForm>,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    CsrfToken::verify_from_session(&session, &form.csrftoken)?;

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let correct_login_code = match LoginCode::get(&session) {
        Ok(Some(e)) => e,
        Ok(None) => {
            // if there's no code, the session is invalid or expired, send back
            // the original form with an error
            return confirm_email_invalid_session_response(&mut conn, &identity, &session, is_hx)
                .await;
        }
        _ => {
            return Err(ErrorInternalServerError(anyhow!(
                "Could not get code from session"
            )))
        }
    };

    // If the login code is wrong, send back the code form with an error.
    if !correct_login_code.verify(&form.code) {
        let csrf_token = CsrfToken::get_or_create(&session)?;
        let body = if *is_hx {
            ProfileConfirmEmailPartial {
                csrf_token,
                error: Some("Invalid code".into()),
            }
            .render()
            .map_err(ErrorInternalServerError)?;
        } else {
            let user = queries::get_user_from_identity(&mut conn, &identity).await?;
            let groups = queries::get_group_links(&mut conn, user.id).await?;
            ProfileConfirmEmailPage {
                title: "Silly Goals".into(),
                csrf_token,
                error: Some("Invalid code".into()),
                user,
                groups,
            }
            .render()
            .map_err(ErrorInternalServerError)?;
        };
        return Ok(HttpResponse::Ok().body(body));
    }

    let change_email = match ChangeEmail::get(&session) {
        Ok(Some(e)) => e,
        Ok(None) => {
            return confirm_email_invalid_session_response(&mut conn, &identity, &session, is_hx)
                .await;
        }
        Err(err) => return Err(ErrorInternalServerError(err)),
    };

    let change_email = change_email.to_string();

    let userid = identity.id().map_err(ErrorInternalServerError)?;
    let user_uuid = Uuid::parse_str(&userid).map_err(ErrorInternalServerError)?;

    sqlx::query!(
        "UPDATE users SET email = $1 WHERE userid = $2;",
        change_email,
        user_uuid
    )
    .execute(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    if *is_hx {
        let body = ProfilePartial { user }
            .render()
            .map_err(ErrorInternalServerError)?;
        let notification = htmx::hx_trigger_notification(
            "Email Updated".into(),
            format!("Your email has been changed to {}", change_email),
            htmx::NotificationVariant::Success,
            true,
        );
        return Ok(HttpResponse::Ok()
            .append_header(notification)
            .append_header(("HX-Trigger", "updateLocation"))
            .body(body));
    }

    let groups = queries::get_group_links(&mut conn, user.id).await?;

    let body = ProfilePage {
        title: "Silly Goals".into(),
        user,
        groups,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}
