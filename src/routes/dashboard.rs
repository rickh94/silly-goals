use actix_identity::Identity;
use actix_session::Session;
use actix_web::{
    error::{ErrorInternalServerError, ErrorNotFound},
    get, post, web, HttpResponse,
};
use askama::Template;
use log::error;
use serde::Deserialize;
use sqlx::PgPool;

use crate::{
    csrf_token::CsrfToken, queries, DeadlineType, Goal, GoalBehavior, Group, GroupDisplay,
    GroupLink, Tone, User,
};

mod filters {
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

    pub fn stage_loop_comp(stage: &i16, index: &usize) -> ::askama::Result<bool> {
        Ok(*stage as usize == *index)
    }

    pub fn stage_text(index: &i16, stages: &Vec<String>) -> ::askama::Result<String> {
        if stages.len() >= *index as usize {
            Ok("unknown".into())
        } else {
            Ok(stages[*index as usize].clone())
        }
    }
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct Dashboard {
    title: String,
    user: User,
    groups: Vec<Group>,
}

#[get("/dashboard")]
async fn dashboard(identity: Identity, pool: web::Data<PgPool>) -> actix_web::Result<Dashboard> {
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

    Ok(Dashboard {
        title: "Dashboard . Silly Goals".into(),
        user,
        groups,
    })
}

#[derive(Template)]
#[template(path = "new_group.html")]
struct NewGroup {
    title: String,
    user: User,
    tones: Vec<Tone>,
    groups: Vec<Group>,
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

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

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

    let groups = sqlx::query_as!(Group, "SELECT * FROM groups WHERE user_id = $1", user.id)
        .fetch_all(&mut conn)
        .await
        .map_err(ErrorInternalServerError)?;

    let csrf_token = CsrfToken::get_or_create(&session).map_err(ErrorInternalServerError)?;

    Ok(NewGroup {
        title: "New Group . Silly Goals".into(),
        user,
        tones,
        csrf_token,
        groups,
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

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

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
    groups: Vec<GroupLink>,
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

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let group = sqlx::query_as!(
        GroupDisplay,
        r#"SELECT 
        g.id,
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

    let mut goals_in_stages = vec![vec![]; 4];

    for goal in goals.iter() {
        if goal.stage < 5 {
            goals_in_stages[goal.stage as usize].push(goal.clone());
        } else {
            error!("Goal has invalid stage, skipping: {:#?}", goal)
        }
    }

    let groups = sqlx::query_as!(
        GroupLink,
        "SELECT id, title FROM groups WHERE user_id = $1",
        user.id
    )
    .fetch_all(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    let body = ShowGroup {
        title: format!("Group {} . Silly Goals", group.title),
        user,
        group,
        goals_in_stages,
        groups,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

#[derive(Debug, Deserialize)]
struct InitialStage {
    stage: Option<usize>,
}

#[derive(Template)]
#[template(path = "new_goal.html")]
struct NewGoal {
    title: String,
    user: User,
    group: GroupDisplay,
    goals_in_stages: Vec<Vec<Goal>>,
    selected_stage: usize,
    csrf_token: CsrfToken,
    groups: Vec<GroupLink>,
}

#[get("/groups/{id}/goals/new")]
async fn new_goal(
    identity: Identity,
    path: web::Path<i64>,
    pool: web::Data<PgPool>,
    query: web::Query<InitialStage>,
    session: Session,
) -> actix_web::Result<HttpResponse> {
    let selected_stage = query.stage.unwrap_or(0);
    let group_id = path.into_inner();
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let groups = sqlx::query_as!(
        GroupLink,
        "SELECT id, title FROM groups WHERE user_id = $1",
        user.id
    )
    .fetch_all(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    let group = sqlx::query_as!(
        GroupDisplay,
        r#"SELECT 
        g.id,
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

    let mut goals_in_stages = vec![vec![]; 4];

    for goal in goals.iter() {
        if goal.stage < 5 {
            goals_in_stages[goal.stage as usize].push(goal.clone());
        } else {
            error!("Goal has invalid stage, skipping: {:#?}", goal)
        }
    }

    let csrf_token = CsrfToken::get_or_create(&session)?;

    let body = NewGoal {
        title: format!("Group {} . Silly Goals", group.title),
        user,
        group,
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
    pool: web::Data<PgPool>,
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

    sqlx::query_as!(
        Goal,
        "INSERT INTO goals(title, description, stage, deadline, group_id) 
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *;",
        form.title,
        form.description,
        form.stage,
        form.deadline,
        group.id,
    )
    .fetch_one(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/groups/{}", group_id)))
        .finish())
}

#[derive(Template)]
#[template(path = "goal.html")]
struct ShowGoal {
    title: String,
    user: User,
    goal: Goal,
    group: GroupDisplay,
    goals_in_stages: Vec<Vec<Goal>>,
    groups: Vec<GroupLink>,
}

#[get("/groups/{group_id}/goals/{goal_id}")]
async fn get_goal(
    identity: Identity,
    path: web::Path<(i64, i64)>,
    pool: web::Data<PgPool>,
) -> actix_web::Result<HttpResponse> {
    let (group_id, goal_id) = path.into_inner();
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let group = sqlx::query_as!(
        GroupDisplay,
        r#"SELECT 
        g.id,
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

    let mut goals_in_stages = vec![vec![]; 4];

    for goal in goals.iter() {
        if goal.stage < 5 {
            goals_in_stages[goal.stage as usize].push(goal.clone());
        } else {
            error!("Goal has invalid stage, skipping: {:#?}", goal)
        }
    }

    let goal = goals.iter().find(|g| g.id == goal_id);

    if goal.is_none() {
        return Err(ErrorNotFound("Goal not found"));
    }

    // just checked for none, now we know it's there
    #[allow(clippy::unwrap_used)]
    let goal = goal.unwrap().clone();

    let groups = sqlx::query_as!(
        GroupLink,
        "SELECT id, title FROM groups WHERE user_id = $1",
        user.id
    )
    .fetch_all(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    let body = ShowGoal {
        title: format!("Group {} . Silly Goals", group.title),
        user,
        goal,
        group,
        goals_in_stages,
        groups,
    }
    .render()
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().body(body))
}

#[derive(Template)]
#[template(path = "edit_goal.html")]
struct EditGoal {
    title: String,
    user: User,
    group: GroupDisplay,
    goals_in_stages: Vec<Vec<Goal>>,
    csrf_token: CsrfToken,
    goal: Goal,
    groups: Vec<GroupLink>,
}

#[get("/groups/{group_id}/goals/{goal_id}/edit")]
async fn edit_goal(
    identity: Identity,
    path: web::Path<(i64, i64)>,
    pool: web::Data<PgPool>,
    session: Session,
) -> actix_web::Result<HttpResponse> {
    let (group_id, goal_id) = path.into_inner();
    let mut conn = pool
        .get_ref()
        .acquire()
        .await
        .map_err(ErrorInternalServerError)?;

    let user = queries::get_user_from_identity(&mut conn, &identity).await?;

    let group = sqlx::query_as!(
        GroupDisplay,
        r#"SELECT 
        g.id,
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

    let mut goals_in_stages = vec![vec![]; 4];

    for goal in goals.iter() {
        if goal.stage < 5 {
            goals_in_stages[goal.stage as usize].push(goal.clone());
        } else {
            error!("Goal has invalid stage, skipping: {:#?}", goal)
        }
    }

    let goal = goals.iter().find(|g| g.id == goal_id);

    if goal.is_none() {
        return Err(ErrorNotFound("Goal not found"));
    }

    // just checked for none, now we know it's there
    #[allow(clippy::unwrap_used)]
    let goal = goal.unwrap().clone();

    let csrf_token = CsrfToken::get_or_create(&session)?;

    let groups = sqlx::query_as!(
        GroupLink,
        "SELECT id, title FROM groups WHERE user_id = $1",
        user.id
    )
    .fetch_all(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    let body = EditGoal {
        title: format!("Group {} . Silly Goals", group.title),
        user,
        group,
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
    pool: web::Data<PgPool>,
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

    sqlx::query_as!(
        Goal,
        "UPDATE goals
        SET (title, description, stage, deadline) =
        ($1, $2, $3, $4)
        WHERE 
        id = $5 AND group_id = $6
        RETURNING *;",
        form.title,
        form.description,
        form.stage,
        form.deadline,
        goal_id,
        group.id,
    )
    .fetch_one(&mut conn)
    .await
    .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", format!("/groups/{}", group_id)))
        .finish())
}
