use std::unreachable;

use actix_identity::Identity;
use actix_session::Session;
use actix_web::{
    delete,
    error::{ErrorBadRequest, ErrorInternalServerError, ErrorNotFound},
    get, patch, post, web, HttpResponse,
};
use askama::Template;
use log::error;
use serde::Deserialize;
use sqlx::{types::Json, SqlitePool};

use crate::{
    csrf_token::CsrfToken,
    htmx::{hx_trigger_notification, HxHeaderInfo},
    htmx::{IsHtmx, NotificationVariant},
    queries, DeadlineType, Goal, GoalBehavior, Group, GroupDisplay, GroupLink, Tone, User,
};

mod filters {
    use chrono::prelude::*;

    use crate::Goal;

    pub fn stage_color<S: PartialEq + std::convert::TryInto<usize> + Clone>(
        s: &S,
    ) -> ::askama::Result<&'static str> {
        let s = (*s).clone();
        match s.try_into().unwrap_or(5) {
            0 => Ok("bg-rose-500"),
            1 => Ok("bg-amber-500"),
            2 => Ok("bg-sky-500"),
            3 => Ok("bg-emerald-500"),
            _ => Ok("bg-gray-500"),
        }
    }

    pub fn stage_color_light<S: PartialEq + std::convert::TryInto<usize> + Clone>(
        s: &S,
    ) -> ::askama::Result<&'static str> {
        let s = (*s).clone();
        match s.try_into().unwrap_or(5) {
            0 => Ok("bg-rose-200"),
            1 => Ok("bg-amber-200"),
            2 => Ok("bg-sky-200"),
            3 => Ok("bg-emerald-200"),
            _ => Ok("bg-gray-200"),
        }
    }
    pub fn stage_border_light<S: PartialEq + std::convert::TryInto<usize> + Clone>(
        s: &S,
    ) -> ::askama::Result<&'static str> {
        let s = (*s).clone();
        match s.try_into().unwrap_or(5) {
            0 => Ok("border-rose-200"),
            1 => Ok("border-amber-200"),
            2 => Ok("border-sky-200"),
            3 => Ok("border-emerald-200"),
            _ => Ok("border-gray-200"),
        }
    }

    pub fn stage_loop_comp(stage: &i64, index: &usize) -> ::askama::Result<bool> {
        Ok(*stage as usize == *index)
    }

    pub fn stage_text<S: std::convert::TryInto<usize> + Clone>(
        index: &S,
        stages: &Vec<String>,
    ) -> ::askama::Result<String> {
        let index = (*index).clone();
        match index.try_into() {
            Ok(x) if x < stages.len() => Ok(stages[x].clone()),
            _ => Ok("unknown".into()),
        }
    }

    pub fn is_past_deadline(goal: &Goal) -> ::askama::Result<bool> {
        if let Some(deadline) = &goal.deadline {
            let deadline = NaiveDate::parse_from_str(&deadline, "%Y-%m-%d")
                .map_err(|err| askama::Error::Custom(err.into()))?;

            let current_date = Utc::now().naive_utc();

            Ok(deadline < current_date.date())
        } else {
            Ok(false)
        }
    }
}

fn group_goals_by_stage(goals: &[Goal]) -> Vec<Vec<Goal>> {
    let mut goals_in_stages = vec![vec![]; 4];

    for goal in goals.iter() {
        if goal.stage < 5 {
            goals_in_stages[goal.stage as usize].push(goal.clone());
        } else {
            error!("Goal has invalid stage, skipping: {:#?}", goal)
        }
    }
    goals_in_stages
}

#[derive(Template)]
#[template(path = "pages/dashboard.html")]
struct DashboardPage {
    title: String,
    user: User,
    groups: Vec<Group>,
}

#[derive(Template)]
#[template(path = "partials/dashboard.html")]
struct DashboardPartial {
    groups: Vec<Group>,
    user: User,
}

#[get("/dashboard")]
async fn dashboard(
    identity: Identity,
    pool: web::Data<SqlitePool>,
    is_hx: IsHtmx,
    hx_headers: HxHeaderInfo,
) -> actix_web::Result<HttpResponse> {
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;
    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let groups = sqlx::query_as!(Group, "SELECT * FROM groups WHERE user_id = $1", user.id)
        .fetch_all(&mut conn)
        .await
        .map_err(ErrorInternalServerError)?;

    let body = if *is_hx && !hx_headers.boosted {
        DashboardPartial { groups, user }
            .render()
            .map_err(ErrorInternalServerError)?
    } else {
        DashboardPage {
            title: "Silly Goals".into(),
            user,
            groups,
        }
        .render()
        .map_err(ErrorInternalServerError)?
    };
    Ok(HttpResponse::Ok()
        .insert_header(("HX-Trigger-After-Swap", "updateLocation"))
        .body(body))
}

#[get("/finish-tutorial")]
async fn finish_tutorial(
    identity: Identity,
    pool: web::Data<SqlitePool>,
    is_hx: IsHtmx,
    hx_headers: HxHeaderInfo,
) -> actix_web::Result<HttpResponse> {
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;
    let mut user = queries::get_user_from_identity(&mut conn, &identity).await?;

    sqlx::query!(
        r#"UPDATE users SET is_new_user = 0 WHERE id = $1;"#,
        user.id
    )
    .execute(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    user.is_new_user = false;

    if *is_hx && !hx_headers.boosted {
        let groups = sqlx::query_as!(Group, "SELECT * FROM groups WHERE user_id = $1", user.id)
            .fetch_all(&mut conn)
            .await
            .map_err(ErrorInternalServerError)?;
        let body = DashboardPartial { groups, user }
            .render()
            .map_err(ErrorInternalServerError)?;
        return Ok(HttpResponse::Ok()
            .insert_header(("HX-Trigger-After-Swap", "updateLocation"))
            .body(body));
    }
    return Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/dashboard"))
        .finish());
}

#[derive(Template)]
#[template(path = "pages/new_group.html")]
struct NewGroupPage {
    title: String,
    user: User,
    tones: Vec<Tone>,
    groups: Vec<Group>,
    csrf_token: CsrfToken,
}

#[derive(Template)]
#[template(path = "partials/new_group.html")]
struct NewGroupPartial {
    tones: Vec<Tone>,
    csrf_token: CsrfToken,
}

#[get("/groups/new")]
async fn new_group(
    identity: Identity,
    pool: web::Data<SqlitePool>,
    session: Session,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let tones = sqlx::query_as!(
        Tone,
        r#"SELECT 
        id, name, stages as "stages: Json<Vec<String>>", deadline as "deadline: DeadlineType", global as "global: bool", 
        greeting, unmet_behavior as "unmet_behavior: GoalBehavior", user_id 
        FROM tones 
        WHERE global = 1 OR user_id = $1;"#,
        user.id
    )
    .fetch_all(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    let csrf_token = CsrfToken::get_or_create(&session).map_err(ErrorInternalServerError)?;

    if *is_hx {
        let body = NewGroupPartial { tones, csrf_token }
            .render()
            .map_err(ErrorInternalServerError)?;
        return Ok(HttpResponse::Ok()
            .insert_header(("HX-Trigger-After-Swap", "updateLocation"))
            .body(body));
    }

    let groups = sqlx::query_as!(Group, "SELECT * FROM groups WHERE user_id = $1", user.id)
        .fetch_all(&mut conn)
        .await
        .map_err(ErrorInternalServerError)?;

    let body = NewGroupPage {
        title: "Silly Goals".into(),
        user,
        tones,
        csrf_token,
        groups,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

#[derive(Deserialize)]
struct GroupForm {
    title: String,
    description: Option<String>,
    tone_id: i64,
    csrftoken: String,
}

#[post("/groups/new")]
async fn post_new_group(
    identity: Identity,
    form: web::Form<GroupForm>,
    session: Session,
    pool: web::Data<SqlitePool>,
) -> actix_web::Result<HttpResponse> {
    CsrfToken::verify_from_session(&session, &form.csrftoken)?;

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let created_group_id = sqlx::query_scalar!(
        "INSERT INTO groups(title, description, tone_id, user_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id;",
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

    let notification = hx_trigger_notification(
        format!("Created {}", form.title),
        "New Group Created!".into(),
        NotificationVariant::Success,
        true,
    );

    Ok(HttpResponse::SeeOther()
        .append_header(notification)
        .append_header(("HX-Trigger-After-Settle", "updateLocation"))
        .append_header(("Location", format!("/groups/{}", created_group_id)))
        .finish())
}

#[derive(Template)]
#[template(path = "pages/dashboard_edit_group.html")]
struct DashboardEditGroupPage {
    title: String,
    user: User,
    group: Group,
    groups: Vec<Group>,
    tones: Vec<Tone>,
    csrf_token: CsrfToken,
}

#[derive(Template)]
#[template(path = "partials/edit_group.html")]
struct EditGroupPartial {
    group: Group,
    tones: Vec<Tone>,
    csrf_token: CsrfToken,
    return_to: String,
}

#[get("/dashboard/groups/{id}/edit")]
async fn dashboard_edit_group(
    identity: Identity,
    path: web::Path<i64>,
    pool: web::Data<SqlitePool>,
    session: Session,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    let group_id = path.into_inner();
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let group = sqlx::query_as!(
        Group,
        r#"SELECT 
        id,
        title,
        description,
        tone_id,
        user_id
        FROM groups
        WHERE user_id = $1 AND id = $2;"#,
        user.id,
        group_id
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|err| match err {
        sqlx::Error::RowNotFound => ErrorNotFound(err),
        e => ErrorInternalServerError(e),
    })?;

    let csrf_token = CsrfToken::get_or_create(&session)?;

    let tones = sqlx::query_as!(
        Tone,
        r#"SELECT 
        id, name, stages as "stages: Json<Vec<String>>", deadline as "deadline: DeadlineType", global as "global: bool", 
        greeting, unmet_behavior as "unmet_behavior: GoalBehavior", user_id 
        FROM tones 
        WHERE global = 1 OR user_id = $1;"#,
        user.id
    )
    .fetch_all(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    if *is_hx {
        let body = EditGroupPartial {
            tones: tones.clone(),
            group: group.clone(),
            csrf_token: csrf_token.clone(),
            return_to: "/dashboard".into(),
        }
        .render()
        .map_err(ErrorInternalServerError)?;
        return Ok(HttpResponse::Ok()
            .insert_header(("HX-Trigger-After-Swap", "updateLocation"))
            .body(body));
    }

    let groups = sqlx::query_as!(Group, "SELECT * FROM groups WHERE user_id = $1", user.id)
        .fetch_all(&mut conn)
        .await
        .map_err(ErrorInternalServerError)?;

    let body = DashboardEditGroupPage {
        title: "Silly Goals".into(),
        user,
        group,
        groups,
        csrf_token,
        tones,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

#[derive(Deserialize)]
struct EditGroupForm {
    title: String,
    description: Option<String>,
    tone_id: i64,
    csrftoken: String,
    return_to: String,
}

/// Edit a group's basic information, either from the group page or the
/// dashboard page
#[post("/groups/{id}/edit")]
async fn post_edit_group(
    identity: Identity,
    path: web::Path<i64>,
    form: web::Form<EditGroupForm>,
    session: Session,
    pool: web::Data<SqlitePool>,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    CsrfToken::verify_from_session(&session, &form.csrftoken)?;
    let group_id = path.into_inner();
    if form.return_to != "/dashboard" && form.return_to != format!("/groups/{}", group_id) {
        return Err(ErrorBadRequest("Invalid return_to"));
    }

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    sqlx::query!(
        "UPDATE groups
        SET 
        title = $1, description = $2, tone_id = $3
        WHERE 
        id = $4 AND user_id = $5;",
        form.title,
        form.description,
        form.tone_id,
        group_id,
        user.id,
    )
    .execute(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    if *is_hx {
        let body = if form.return_to == "/dashboard" {
            let groups = sqlx::query_as!(Group, "SELECT * FROM groups WHERE user_id = $1", user.id)
                .fetch_all(&mut conn)
                .await
                .map_err(ErrorInternalServerError)?;
            DashboardPartial { groups, user }
                .render()
                .map_err(ErrorInternalServerError)?
        } else if form.return_to == format!("/groups/{}", group_id) {
            let group = queries::get_group_with_info(&mut conn, user.id, group_id).await?;
            let goals = queries::get_goals_for_group(&mut conn, group.id).await?;
            let goals_in_stages = group_goals_by_stage(&goals);

            ShowGroupPartial {
                group: group.into(),
                goals_in_stages,
            }
            .render()
            .map_err(ErrorInternalServerError)?
        } else {
            unreachable!();
        };
        let notification = hx_trigger_notification(
            format!("{} Updated", form.title),
            "Your group has been updated".into(),
            NotificationVariant::Success,
            true,
        );
        return Ok(HttpResponse::Ok()
            .insert_header(notification)
            .append_header(("HX-Trigger-After-Settle", "updateLocation"))
            .body(body));
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", form.return_to.to_owned()))
        .finish())
}

#[derive(Template)]
#[template(path = "pages/group.html")]
struct ShowGroupPage {
    title: String,
    user: User,
    group: GroupDisplay,
    goals_in_stages: Vec<Vec<Goal>>,
    groups: Vec<GroupLink>,
}

#[derive(Template)]
#[template(path = "partials/group.html")]
struct ShowGroupPartial {
    group: GroupDisplay,
    goals_in_stages: Vec<Vec<Goal>>,
}

/// Get a group and its goals by the group id
#[get("/groups/{id}")]
async fn get_group(
    identity: Identity,
    path: web::Path<i64>,
    pool: web::Data<SqlitePool>,
    is_hx: IsHtmx,
    hx_header: HxHeaderInfo,
) -> actix_web::Result<HttpResponse> {
    let group_id = path.into_inner();
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let group = queries::get_group_with_info(&mut conn, user.id, group_id).await?;

    let goals = queries::get_goals_for_group(&mut conn, group_id).await?;

    let goals_in_stages = group_goals_by_stage(&goals);

    if *is_hx && !hx_header.boosted {
        let body = ShowGroupPartial {
            group: group.into(),
            goals_in_stages,
        }
        .render()
        .map_err(ErrorInternalServerError)?;
        return Ok(HttpResponse::Ok().body(body));
    }

    let groups = queries::get_group_links(&mut conn, user.id).await?;

    let body = ShowGroupPage {
        title: "Silly Goals".into(),
        user,
        group: group.into(),
        goals_in_stages,
        groups,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

/// Delete a group and all its goals
#[delete("/groups/{id}")]
async fn delete_group(
    identity: Identity,
    path: web::Path<i64>,
    pool: web::Data<SqlitePool>,
) -> actix_web::Result<HttpResponse> {
    let group_id = path.into_inner();
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    sqlx::query!(
        r#"DELETE FROM groups WHERE user_id = $1 AND id = $2;"#,
        user.id,
        group_id
    )
    .execute(&mut conn)
    .await
    .map_err(|err| match err {
        sqlx::Error::RowNotFound => ErrorNotFound(err),
        e => ErrorInternalServerError(e),
    })?;

    Ok(HttpResponse::Ok().finish())
}

#[derive(Debug, Deserialize)]
struct InitialStage {
    stage: Option<usize>,
}

#[derive(Template)]
#[template(path = "pages/new_goal.html")]
struct NewGoalPage {
    title: String,
    user: User,
    group: GroupDisplay,
    goals_in_stages: Vec<Vec<Goal>>,
    selected_stage: usize,
    csrf_token: CsrfToken,
    groups: Vec<GroupLink>,
}

#[derive(Template)]
#[template(path = "partials/new_goal.html")]
struct NewGoalPartial {
    group: GroupDisplay,
    selected_stage: usize,
    csrf_token: CsrfToken,
}

#[get("/groups/{id}/goals/new")]
async fn new_goal(
    identity: Identity,
    path: web::Path<i64>,
    pool: web::Data<SqlitePool>,
    query: web::Query<InitialStage>,
    session: Session,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    let selected_stage = query.stage.unwrap_or(0);
    let group_id = path.into_inner();
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let groups = queries::get_group_links(&mut conn, user.id).await?;

    let group = queries::get_group_with_info(&mut conn, user.id, group_id).await?;
    let csrf_token = CsrfToken::get_or_create(&session)?;

    if *is_hx {
        let body = NewGoalPartial {
            group: group.into(),
            csrf_token,
            selected_stage,
        }
        .render()
        .map_err(ErrorInternalServerError)?;

        return Ok(HttpResponse::Ok()
            .insert_header(("HX-Trigger-After-Swap", "updateLocation"))
            .body(body));
    }

    let goals = queries::get_goals_for_group(&mut conn, group_id).await?;
    let goals_in_stages = group_goals_by_stage(&goals);

    let body = NewGoalPage {
        title: "Silly Goals".into(),
        user,
        group: group.into(),
        goals_in_stages,
        selected_stage,
        csrf_token,
        groups,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

#[derive(Clone, Deserialize)]
struct NewGoalForm {
    title: String,
    description: Option<String>,
    deadline: Option<chrono::NaiveDate>,
    stage: i16,
    csrftoken: String,
}

#[post("/groups/{id}/goals/new")]
async fn post_new_goal(
    identity: Identity,
    path: web::Path<i64>,
    form: web::Form<NewGoalForm>,
    session: Session,
    pool: web::Data<SqlitePool>,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    CsrfToken::verify_from_session(&session, &form.csrftoken)?;
    let group_id = path.into_inner();

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let group = sqlx::query_as!(
        Group,
        r#"SELECT * FROM groups 
        WHERE user_id = $1 AND id = $2;"#,
        user.id,
        group_id
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|err| match err {
        sqlx::Error::RowNotFound => ErrorNotFound(err),
        e => ErrorInternalServerError(e),
    })?;

    sqlx::query!(
        "INSERT INTO goals(title, description, stage, deadline, group_id) 
        VALUES ($1, $2, $3, $4, $5)",
        form.title,
        form.description,
        form.stage,
        form.deadline,
        group.id,
    )
    .execute(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    if *is_hx {
        let group = queries::get_group_with_info(&mut conn, user.id, group.id).await?;
        let goals = queries::get_goals_for_group(&mut conn, group.id).await?;
        let goals_in_stages = group_goals_by_stage(&goals);

        let notification = hx_trigger_notification(
            format!("Created {}", form.title),
            "Your goal has been created".into(),
            NotificationVariant::Success,
            true,
        );

        let body = ShowGroupPartial {
            group: group.into(),
            goals_in_stages,
        }
        .render()
        .map_err(ErrorInternalServerError)?;
        Ok(HttpResponse::Ok()
            .append_header(notification)
            .append_header(("HX-Trigger-After-Settle", "updateLocation"))
            .body(body))
    } else {
        Ok(HttpResponse::SeeOther()
            .insert_header(("Location", format!("/groups/{}", group_id)))
            .finish())
    }
}

#[derive(Template)]
#[template(path = "pages/goal.html")]
struct ShowGoalPage {
    title: String,
    user: User,
    goal: Goal,
    group: GroupDisplay,
    goals_in_stages: Vec<Vec<Goal>>,
    groups: Vec<GroupLink>,
}

#[derive(Template)]
#[template(path = "partials/goal.html")]
struct ShowGoalPartial {
    goal: Goal,
    group: GroupDisplay,
}

#[get("/groups/{group_id}/goals/{goal_id}")]
async fn get_goal(
    identity: Identity,
    path: web::Path<(i64, i64)>,
    pool: web::Data<SqlitePool>,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    let (group_id, goal_id) = path.into_inner();
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let group = queries::get_group_with_info(&mut conn, user.id, group_id).await?;

    if *is_hx {
        let goal = sqlx::query_as!(
            Goal,
            "SELECT * FROM goals WHERE id = $1 AND group_id = $2;",
            goal_id,
            group_id,
        )
        .fetch_one(&mut conn)
        .await
        .map_err(|err| match err {
            sqlx::Error::RowNotFound => ErrorNotFound(err),
            _ => ErrorInternalServerError(err),
        })?;

        let body = ShowGoalPartial {
            goal,
            group: group.into(),
        }
        .render()
        .map_err(ErrorInternalServerError)?;

        return Ok(HttpResponse::Ok().body(body));
    }

    let goals = queries::get_goals_for_group(&mut conn, group_id).await?;
    let goals_in_stages = group_goals_by_stage(&goals);

    let goal = goals.iter().find(|g| g.id == goal_id);

    if goal.is_none() {
        return Err(ErrorNotFound("Goal not found"));
    }

    // just checked for none, now we know it's there
    #[allow(clippy::unwrap_used)]
    let goal = goal.unwrap().clone();

    let groups = queries::get_group_links(&mut conn, user.id).await?;

    let body = ShowGoalPage {
        title: "Silly Goals".into(),
        user,
        goal,
        group: group.into(),
        goals_in_stages,
        groups,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

#[derive(Template)]
#[template(path = "pages/edit_goal.html")]
struct EditGoalPage {
    title: String,
    user: User,
    group: GroupDisplay,
    goals_in_stages: Vec<Vec<Goal>>,
    csrf_token: CsrfToken,
    goal: Goal,
    groups: Vec<GroupLink>,
}

#[derive(Template)]
#[template(path = "partials/edit_goal.html")]
struct EditGoalPartial {
    group: GroupDisplay,
    csrf_token: CsrfToken,
    goal: Goal,
}

#[get("/groups/{group_id}/goals/{goal_id}/edit")]
async fn edit_goal(
    identity: Identity,
    path: web::Path<(i64, i64)>,
    pool: web::Data<SqlitePool>,
    session: Session,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    let (group_id, goal_id) = path.into_inner();
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let group = queries::get_group_with_info(&mut conn, user.id, group_id).await?;
    let csrf_token = CsrfToken::get_or_create(&session)?;

    if *is_hx {
        let goal = sqlx::query_as!(
            Goal,
            "SELECT * FROM goals WHERE id = $1 AND group_id = $2",
            goal_id,
            group_id,
        )
        .fetch_one(&mut conn)
        .await
        .map_err(ErrorInternalServerError)?;

        let body = EditGoalPartial {
            goal,
            group: group.into(),
            csrf_token,
        }
        .render()
        .map_err(ErrorInternalServerError)?;

        return Ok(HttpResponse::Ok().body(body));
    }

    let goals = queries::get_goals_for_group(&mut conn, group_id).await?;
    let goals_in_stages = group_goals_by_stage(&goals);

    let goal = goals.iter().find(|g| g.id == goal_id);

    if goal.is_none() {
        return Err(ErrorNotFound("Goal not found"));
    }

    // just checked for none, now we know it's there
    #[allow(clippy::unwrap_used)]
    let goal = goal.unwrap().clone();

    let groups = queries::get_group_links(&mut conn, user.id).await?;

    let body = EditGoalPage {
        title: "Silly Goals".into(),
        user,
        group: group.into(),
        goals_in_stages,
        csrf_token,
        goal,
        groups,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

#[derive(Deserialize)]
struct EditGoalForm {
    title: String,
    description: Option<String>,
    deadline: Option<chrono::NaiveDate>,
    stage: i16,
    csrftoken: String,
}

#[post("/groups/{group_id}/goals/{goal_id}/edit")]
async fn post_edit_goal(
    identity: Identity,
    path: web::Path<(i64, i64)>,
    form: web::Form<EditGoalForm>,
    session: Session,
    pool: web::Data<SqlitePool>,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    CsrfToken::verify_from_session(&session, &form.csrftoken)?;
    let (group_id, goal_id) = path.into_inner();

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let group = sqlx::query_as!(
        Group,
        r#"SELECT * FROM groups 
        WHERE user_id = $1 AND id = $2;"#,
        user.id,
        group_id
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|err| match err {
        sqlx::Error::RowNotFound => ErrorNotFound(err),
        e => ErrorInternalServerError(e),
    })?;

    sqlx::query!(
        "UPDATE goals
        SET (title, description, stage, deadline) =
        ($1, $2, $3, $4)
        WHERE 
        id = $5 AND group_id = $6;",
        form.title,
        form.description,
        form.stage,
        form.deadline,
        goal_id,
        group.id,
    )
    .execute(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    if *is_hx {
        let group = queries::get_group_with_info(&mut conn, user.id, group.id).await?;
        let goals = queries::get_goals_for_group(&mut conn, group.id).await?;
        let goals_in_stages = group_goals_by_stage(&goals);
        let notification = hx_trigger_notification(
            format!("{} updated", form.title),
            "Your goal was updated".into(),
            NotificationVariant::Success,
            true,
        );

        let body = ShowGroupPartial {
            group: group.into(),
            goals_in_stages,
        }
        .render()
        .map_err(ErrorInternalServerError)?;

        return Ok(HttpResponse::Ok().append_header(notification).body(body));
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/groups/{}", group_id)))
        .finish())
}

#[derive(Debug, Deserialize)]
struct NewStage {
    stage: i64,
}

#[derive(Template)]
#[template(path = "partials/single_goal_card.html")]
struct SingleGoalCard {
    stage_number: i64,
    goal: Goal,
    group: Group,
}

#[patch("/groups/{group_id}/goals/{goal_id}/stage")]
async fn patch_goal_tone(
    identity: Identity,
    path: web::Path<(i64, i64)>,
    query: web::Query<NewStage>,
    pool: web::Data<SqlitePool>,
) -> actix_web::Result<HttpResponse> {
    let (group_id, goal_id) = path.into_inner();

    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let group = sqlx::query_as!(
        Group,
        r#"SELECT * FROM groups 
        WHERE user_id = $1 AND id = $2;"#,
        user.id,
        group_id
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|err| match err {
        sqlx::Error::RowNotFound => ErrorNotFound(err),
        e => ErrorInternalServerError(e),
    })?;

    if query.stage > 4 || query.stage < 0 {
        return Err(ErrorBadRequest("Stage must be between 0 and 4"));
    }

    sqlx::query!(
        "UPDATE goals
        SET stage = $1 
        WHERE 
        id = $2 AND group_id = $3;",
        query.stage,
        goal_id,
        group_id,
    )
    .execute(&mut conn)
    .await
    .map_err(|err| {
        error!("Could not update database");
        ErrorInternalServerError(err)
    })?;

    let goal = sqlx::query_as!(
        Goal,
        "SELECT * FROM goals WHERE id = $1 AND group_id = $2",
        goal_id,
        group_id
    )
    .fetch_one(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    let body = SingleGoalCard {
        goal,
        group,
        stage_number: query.stage,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

#[delete("/groups/{group_id}/goals/{goal_id}")]
async fn delete_goal(
    identity: Identity,
    path: web::Path<(i64, i64)>,
    pool: web::Data<SqlitePool>,
) -> actix_web::Result<HttpResponse> {
    let (group_id, goal_id) = path.into_inner();
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    sqlx::query!(
        r#"SELECT 
        id
        FROM groups 
        WHERE user_id = $1 AND id = $2;"#,
        user.id,
        group_id
    )
    .fetch_one(&mut conn)
    .await
    .map_err(|err| match err {
        sqlx::Error::RowNotFound => ErrorNotFound(err),
        e => ErrorInternalServerError(e),
    })?;

    sqlx::query!(
        "DELETE FROM goals WHERE group_id = $1 AND id = $2",
        group_id,
        goal_id
    )
    .execute(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().finish())
}

#[derive(Template)]
#[template(path = "pages/group_edit_group.html")]
struct GroupEditGroupPage {
    title: String,
    user: User,
    group: GroupDisplay,
    groups: Vec<GroupLink>,
    goals_in_stages: Vec<Vec<Goal>>,
    tones: Vec<Tone>,
    csrf_token: CsrfToken,
    return_to: String,
}

#[get("/groups/{id}/edit")]
async fn group_edit_group(
    identity: Identity,
    path: web::Path<i64>,
    pool: web::Data<SqlitePool>,
    session: Session,
    is_hx: IsHtmx,
) -> actix_web::Result<HttpResponse> {
    let group_id = path.into_inner();
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let group = queries::get_group_with_info(&mut conn, user.id, group_id).await?;

    let csrf_token = CsrfToken::get_or_create(&session)?;

    let tones = sqlx::query_as!(
        Tone,
        r#"SELECT 
        id, name, stages as "stages: Json<Vec<String>>", deadline as "deadline: DeadlineType", global as "global: bool", 
        greeting, unmet_behavior as "unmet_behavior: GoalBehavior", user_id 
        FROM tones 
        WHERE global = 1 OR user_id = $1;"#,
        user.id
    )
    .fetch_all(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    if *is_hx {
        let body = EditGroupPartial {
            tones: tones.clone(),
            group: group.into(),
            csrf_token: csrf_token.clone(),
            return_to: format!("/groups/{}", group_id),
        }
        .render()
        .map_err(ErrorInternalServerError)?;
        return Ok(HttpResponse::Ok()
            .insert_header(("HX-Trigger-After-Swap", "updateLocation"))
            .body(body));
    }

    let groups = queries::get_group_links(&mut conn, user.id).await?;

    let goals = queries::get_goals_for_group(&mut conn, group_id).await?;

    let goals_in_stages = group_goals_by_stage(&goals);

    let body = GroupEditGroupPage {
        title: "Silly Goals".into(),
        user,
        group: group.clone().into(),
        groups,
        csrf_token,
        tones,
        return_to: format!("/groups/{}", group_id),
        goals_in_stages,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}
