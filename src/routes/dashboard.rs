use actix_identity::Identity;
use actix_session::Session;
use actix_web::{
    error::{ErrorInternalServerError, ErrorNotFound, ErrorUnauthorized},
    get, post, web, HttpResponse,
};
use askama::Template;
use serde::Deserialize;
use shuttle_runtime::tracing::error;
use sqlx::PgPool;

use crate::{
    csrf_token::CsrfToken, DeadlineType, Goal, GoalBehavior, Group, GroupDisplay, Tone, User,
};

#[derive(Template)]
#[template(path = "dashboard.html")]
struct Dashboard {
    pub title: String,
    pub user: User,
}

#[get("/dashboard")]
async fn dashboard(identity: Identity, pool: web::Data<PgPool>) -> actix_web::Result<Dashboard> {
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

    Ok(Dashboard {
        title: "Dashboard . Silly Goals".into(),
        user,
    })
}

#[derive(Template)]
#[template(path = "new_group.html")]
struct NewGroup {
    title: String,
    user: User,
    tones: Vec<Tone>,
    csrf_token: CsrfToken,
}

#[get("/groups/new")]
async fn new_group(
    identity: Identity,
    pool: web::Data<PgPool>,
    session: Session,
) -> actix_web::Result<NewGroup> {
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let userid = identity.id().map_err(ErrorInternalServerError)?;
    let user = sqlx::query_as!(
        User,
        "SELECT id, name, userid, email FROM users
            WHERE userid = $1",
        userid,
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

    let tones = sqlx::query_as!(
        Tone,
        r#"SELECT 
        id, name, stages, deadline as "deadline: DeadlineType", global, 
        greeting, unmet_behavior as "unmet_behavior: GoalBehavior", user_id 
        FROM tones 
        WHERE global = 'true' OR user_id = $1;"#,
        user.id
    )
    .fetch_all(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    let csrf_token = CsrfToken::get_or_create(&session).map_err(ErrorInternalServerError)?;

    Ok(NewGroup {
        title: "New Group . Silly Goals".into(),
        user,
        tones,
        csrf_token,
    })
}

#[derive(Deserialize)]
struct NewGroupForm {
    title: String,
    description: Option<String>,
    tone_id: i64,
    csrftoken: String,
}

#[post("/groups/new")]
async fn post_new_group(
    identity: Identity,
    form: web::Form<NewGroupForm>,
    session: Session,
    pool: web::Data<PgPool>,
) -> actix_web::Result<HttpResponse> {
    CsrfToken::verify_from_session(&session, &form.csrftoken)?;

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let userid = identity.id().map_err(ErrorInternalServerError)?;
    let user = sqlx::query_as!(User, "SELECT * FROM users WHERE userid = $1", userid)
        .fetch_one(&mut conn)
        .await
        .map_err(ErrorInternalServerError)?;

    let created_group = sqlx::query_as!(
        Group,
        "INSERT INTO groups(title, description, tone_id, user_id) 
        VALUES ($1, $2, $3, $4) 
        RETURNING *;",
        form.title,
        form.description,
        form.tone_id,
        user.id
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|err| {
        error!("Could not insert record: {}", err);
        ErrorInternalServerError(err)
    })?;

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/groups/{}", created_group.id)))
        .finish())
}

#[derive(Template)]
#[template(path = "group.html")]
struct ShowGroup {
    title: String,
    user: User,
    group: GroupDisplay,
    goals_in_stages: Vec<Vec<Goal>>,
}

#[get("/groups/{id}")]
async fn get_group(
    identity: Identity,
    path: web::Path<i64>,
    pool: web::Data<PgPool>,
) -> actix_web::Result<HttpResponse> {
    let group_id = path.into_inner();
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let userid = identity.id().map_err(ErrorInternalServerError)?;
    let user = sqlx::query_as!(User, "SELECT * FROM users WHERE userid = $1;", userid)
        .fetch_one(&mut conn)
        .await
        .map_err(ErrorInternalServerError)?;

    let group = sqlx::query_as!(
        GroupDisplay,
        r#"SELECT 
        g.title, 
        g.description, 
        t.name as tone_name, 
        t.stages as tone_stages, 
        t.greeting, 
        t.unmet_behavior as "unmet_behavior: GoalBehavior", 
        t.deadline as "deadline: DeadlineType"
        FROM groups g
        LEFT JOIN tones t
        ON g.tone_id = t.id
        WHERE g.user_id = $1 AND g.id = $2;"#,
        user.id,
        group_id
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|err| match err {
        sqlx::Error::RowNotFound => ErrorNotFound(err),
        e => ErrorInternalServerError(e),
    })?;

    let goals = sqlx::query_as!(Goal, "SELECT * FROM goals WHERE group_id = $1;", group_id)
        .fetch_all(&mut conn)
        .await
        .map_err(ErrorInternalServerError)?;

    let mut goals_in_stages: Vec<Vec<Goal>> = Vec::with_capacity(4);

    for goal in goals.iter() {
        goals_in_stages[goal.stage as usize].push(goal.clone());
    }

    let body = ShowGroup {
        title: format!("Group {} . Silly Goals", group.title),
        user,
        group,
        goals_in_stages,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}
