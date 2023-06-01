use askama::Template;

use crate::{
    csrf_token::CsrfToken, DeadlineType, Goal, Group, GroupDisplay, GroupLink, Tone, User,
};

mod filters {
    use anyhow::anyhow;
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

    pub fn icon_from_word<S: ToString>(s: S) -> ::askama::Result<String> {
        if let Some(c) = s.to_string().chars().next() {
            Ok(format!("{}", c).to_uppercase())
        } else {
            Err(askama::Error::Custom(
                anyhow!("Does not support empty string").into(),
            ))
        }
    }
}

#[derive(Template)]
#[template(path = "pages/dashboard.html")]
pub struct DashboardPage {
    pub title: String,
    pub user: User,
    pub groups: Vec<Group>,
}

#[derive(Template)]
#[template(path = "partials/dashboard.html")]
pub struct DashboardPartial {
    pub groups: Vec<Group>,
    pub user: User,
}

#[derive(Template)]
#[template(path = "pages/new_group.html")]
pub struct NewGroupPage {
    pub title: String,
    pub user: User,
    pub tones: Vec<Tone>,
    pub groups: Vec<Group>,
    pub csrf_token: CsrfToken,
}

#[derive(Template)]
#[template(path = "partials/new_group.html")]
pub struct NewGroupPartial {
    pub tones: Vec<Tone>,
    pub csrf_token: CsrfToken,
}

#[derive(Template)]
#[template(path = "pages/dashboard_edit_group.html")]
pub struct DashboardEditGroupPage {
    pub title: String,
    pub user: User,
    pub group: Group,
    pub groups: Vec<Group>,
    pub tones: Vec<Tone>,
    pub csrf_token: CsrfToken,
}

#[derive(Template)]
#[template(path = "partials/edit_group.html")]
pub struct EditGroupPartial {
    pub group: Group,
    pub tones: Vec<Tone>,
    pub csrf_token: CsrfToken,
    pub return_to: String,
}

#[derive(Template)]
#[template(path = "pages/group.html")]
pub struct ShowGroupPage {
    pub title: String,
    pub user: User,
    pub group: GroupDisplay,
    pub goals_in_stages: Vec<Vec<Goal>>,
    pub groups: Vec<GroupLink>,
}

#[derive(Template)]
#[template(path = "partials/group.html")]
pub struct ShowGroupPartial {
    pub group: GroupDisplay,
    pub goals_in_stages: Vec<Vec<Goal>>,
}

#[derive(Template)]
#[template(path = "pages/new_goal.html")]
pub struct NewGoalPage {
    pub title: String,
    pub user: User,
    pub group: GroupDisplay,
    pub goals_in_stages: Vec<Vec<Goal>>,
    pub selected_stage: usize,
    pub csrf_token: CsrfToken,
    pub groups: Vec<GroupLink>,
}

#[derive(Template)]
#[template(path = "partials/new_goal.html")]
pub struct NewGoalPartial {
    pub group: GroupDisplay,
    pub selected_stage: usize,
    pub csrf_token: CsrfToken,
}

#[derive(Template)]
#[template(path = "pages/goal.html")]
pub struct ShowGoalPage {
    pub title: String,
    pub user: User,
    pub goal: Goal,
    pub group: GroupDisplay,
    pub goals_in_stages: Vec<Vec<Goal>>,
    pub groups: Vec<GroupLink>,
}

#[derive(Template)]
#[template(path = "partials/goal.html")]
pub struct ShowGoalPartial {
    pub goal: Goal,
    pub group: GroupDisplay,
}

#[derive(Template)]
#[template(path = "pages/edit_goal.html")]
pub struct EditGoalPage {
    pub title: String,
    pub user: User,
    pub group: GroupDisplay,
    pub goals_in_stages: Vec<Vec<Goal>>,
    pub csrf_token: CsrfToken,
    pub goal: Goal,
    pub groups: Vec<GroupLink>,
}

#[derive(Template)]
#[template(path = "partials/edit_goal.html")]
pub struct EditGoalPartial {
    pub group: GroupDisplay,
    pub csrf_token: CsrfToken,
    pub goal: Goal,
}

#[derive(Template)]
#[template(path = "partials/single_goal_card.html")]
pub struct SingleGoalCard {
    pub stage_number: i64,
    pub goal: Goal,
    pub group: GroupDisplay,
}

#[derive(Template)]
#[template(path = "pages/group_edit_group.html")]
pub struct GroupEditGroupPage {
    pub title: String,
    pub user: User,
    pub group: GroupDisplay,
    pub groups: Vec<GroupLink>,
    pub goals_in_stages: Vec<Vec<Goal>>,
    pub tones: Vec<Tone>,
    pub csrf_token: CsrfToken,
    pub return_to: String,
}

#[derive(Template)]
#[template(path = "register.html")]
pub struct RegisterStart {
    pub title: String,
    pub csrf_token: CsrfToken,
}

#[derive(Template)]
#[template(path = "register_finish.html")]
pub struct RegisterFinish {
    pub title: String,
    pub csrf_token: CsrfToken,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginStart {
    pub title: String,
    pub csrf_token: CsrfToken,
}

#[derive(Template)]
#[template(path = "login_select.html")]
pub struct LoginSelect {
    pub title: String,
}

#[derive(Template)]
#[template(path = "login_finish.html")]
pub struct LoginFinish {
    pub title: String,
    pub csrf_token: CsrfToken,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/profile.html")]
pub struct ProfilePage {
    pub title: String,
    pub user: User,
    pub groups: Vec<GroupLink>,
}

#[derive(Template)]
#[template(path = "partials/profile.html")]
pub struct ProfilePartial {
    pub user: User,
}

#[derive(Template)]
#[template(path = "pages/profile_edit_name.html")]
pub struct ProfileEditNamePage {
    pub title: String,
    pub user: User,
    pub groups: Vec<GroupLink>,
    pub csrf_token: CsrfToken,
}

#[derive(Template)]
#[template(path = "partials/profile_edit_name.html")]
pub struct ProfileEditNamePartial {
    pub user: User,
    pub csrf_token: CsrfToken,
}

#[derive(Template)]
#[template(path = "pages/profile_edit_email.html")]
pub struct ProfileEditEmailPage {
    pub title: String,
    pub user: User,
    pub groups: Vec<GroupLink>,
    pub csrf_token: CsrfToken,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "partials/profile_edit_email.html")]
pub struct ProfileEditEmailPartial {
    pub user: User,
    pub csrf_token: CsrfToken,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/profile_confirm_email.html")]
pub struct ProfileConfirmEmailPage {
    pub title: String,
    pub user: User,
    pub groups: Vec<GroupLink>,
    pub csrf_token: CsrfToken,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "partials/profile_confirm_email.html")]
pub struct ProfileConfirmEmailPartial {
    pub csrf_token: CsrfToken,
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "partials/dashboard_help_walkthrough.html")]
pub struct DashboardWalkthroughPartial {}

#[derive(Template)]
#[template(path = "pages/dashboard_help_walkthrough.html")]
pub struct DashboardWalkthroughPage {
    pub title: String,
    pub user: User,
    pub groups: Vec<GroupLink>,
}

#[derive(Template)]
#[template(path = "partials/dashboard_help_general.html")]
pub struct DashboardGeneralHelpPartial {}

#[derive(Template)]
#[template(path = "pages/dashboard_help_general.html")]
pub struct DashboardGeneralHelpPage {
    pub title: String,
    pub user: User,
    pub groups: Vec<GroupLink>,
}

#[derive(Template)]
#[template(path = "partials/dashboard_help_tones.html")]
pub struct DashboardTonesPartial {}

#[derive(Template)]
#[template(path = "pages/dashboard_help_tones.html")]
pub struct DashboardTonesPage {
    pub title: String,
    pub user: User,
    pub groups: Vec<GroupLink>,
}
