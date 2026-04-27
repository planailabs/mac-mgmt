use dioxus::prelude::*;

use super::skill_center_list::SkillCenterRow;

#[server]
async fn get_skill_center(id: String) -> Result<Option<SkillCenterRow>, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|_| ServerFnError::new("invalid UUID"))?;

    let row = sqlx::query_as::<_, SkillCenterRow>(
        "SELECT id, name, url, priority, enabled, created_at, updated_at \
         FROM skill_centers WHERE id = $1",
    )
    .bind(uuid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(format!("query failed: {e}")))?;

    Ok(row)
}

#[server]
async fn upsert_skill_center(
    id: Option<String>,
    name: String,
    url: String,
    federation_token: String,
    priority: i32,
    enabled: bool,
) -> Result<SkillCenterRow, ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    if let Some(id_str) = id {
        let uuid: uuid::Uuid = id_str
            .parse()
            .map_err(|_| ServerFnError::new("invalid UUID"))?;
        let row = sqlx::query_as::<_, SkillCenterRow>(
            "UPDATE skill_centers SET name = $1, url = $2, federation_token = $3, \
             priority = $4, enabled = $5, updated_at = now() \
             WHERE id = $6 \
             RETURNING id, name, url, priority, enabled, created_at, updated_at",
        )
        .bind(&name)
        .bind(&url)
        .bind(&federation_token)
        .bind(priority)
        .bind(enabled)
        .bind(uuid)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(format!("update failed: {e}")))?;
        Ok(row)
    } else {
        let row = sqlx::query_as::<_, SkillCenterRow>(
            "INSERT INTO skill_centers (name, url, federation_token, priority, enabled) \
             VALUES ($1, $2, $3, $4, $5) \
             RETURNING id, name, url, priority, enabled, created_at, updated_at",
        )
        .bind(&name)
        .bind(&url)
        .bind(&federation_token)
        .bind(priority)
        .bind(enabled)
        .fetch_one(&pool)
        .await
        .map_err(|e| ServerFnError::new(format!("insert failed: {e}")))?;
        Ok(row)
    }
}

#[server]
async fn delete_skill_center(id: String) -> Result<(), ServerFnError> {
    let user = crate::web::user::current_user().await?;
    user.require_admin()?;
    let pool = crate::server_pool()?;

    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|_| ServerFnError::new("invalid UUID"))?;

    sqlx::query("DELETE FROM skill_centers WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(format!("delete failed: {e}")))?;

    Ok(())
}

#[component]
pub fn SkillCenterDetail(id: String) -> Element {
    let center_future = use_server_future(move || {
        let id = id.clone();
        async move { get_skill_center(id).await }
    })?;

    rsx! {
        div { class: "px-6 py-8 max-w-3xl mx-auto",
            match &*center_future.read() {
                Some(Ok(Some(center))) => rsx! {
                    h1 { class: "text-2xl font-bold mb-4 dark:text-white", "{center.name}" }
                    dl { class: "grid grid-cols-2 gap-2 text-sm",
                        dt { class: "font-medium dark:text-gray-300", "URL" }
                        dd { class: "dark:text-gray-400", "{center.url}" }
                        dt { class: "font-medium dark:text-gray-300", "Priority" }
                        dd { class: "dark:text-gray-400", "{center.priority}" }
                        dt { class: "font-medium dark:text-gray-300", "Enabled" }
                        dd { class: "dark:text-gray-400",
                            if center.enabled { "Yes" } else { "No" }
                        }
                        dt { class: "font-medium dark:text-gray-300", "Created" }
                        dd { class: "dark:text-gray-400", "{center.created_at}" }
                    }
                },
                Some(Ok(None)) => rsx! { p { class: "text-red-500", "Skill center not found." } },
                Some(Err(e)) => rsx! { p { class: "text-red-500", "Error: {e}" } },
                None => rsx! { p { class: "text-gray-500", "Loading..." } },
            }
        }
    }
}
