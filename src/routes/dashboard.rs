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
    queries,
    templates::*,
    DeadlineType, Goal, GoalBehavior, Group, Tone,
};

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

    let group = queries::get_group_with_info(&mut conn, user.id, group_id).await?;

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
        group: group.into(),
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
