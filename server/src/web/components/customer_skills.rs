use dioxus::prelude::*;

use super::bundle_detail::SkillChannelDisplay;

/// Direct skill assignment display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct CustomerSkillDisplay {
    pub customer_skill_id: uuid::Uuid,
    pub skill_slug: String,
    pub channel: String,
}

/// Skill coming from a bundle (read-only).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct BundleSkillDisplay {
    pub skill_slug: String,
    pub channel: String,
    pub bundle_slug: String,
}

/// Bundle assignment display.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct CustomerBundleDisplay {
    pub customer_bundle_id: uuid::Uuid,
    pub bundle_slug: String,
    pub bundle_name: String,
}

#[server]
async fn list_customer_skills(customer_id: String) -> Result<Vec<CustomerSkillDisplay>, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = customer_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let skills = sqlx::query_as::<_, CustomerSkillDisplay>(
        "SELECT cs.id as customer_skill_id, s.slug as skill_slug, sc.channel \
         FROM customer_skills cs \
         JOIN skill_channels sc ON sc.id = cs.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE cs.customer_id = $1 \
         ORDER BY s.slug, sc.channel",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(skills)
}

#[server]
async fn list_customer_bundles(customer_id: String) -> Result<Vec<CustomerBundleDisplay>, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = customer_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let bundles = sqlx::query_as::<_, CustomerBundleDisplay>(
        "SELECT cb.id as customer_bundle_id, b.slug as bundle_slug, b.name as bundle_name \
         FROM customer_bundles cb \
         JOIN bundles b ON b.id = cb.bundle_id \
         WHERE cb.customer_id = $1 \
         ORDER BY b.slug",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(bundles)
}

#[server]
async fn list_bundle_skills(customer_id: String) -> Result<Vec<BundleSkillDisplay>, ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = customer_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let rows = sqlx::query_as::<_, BundleSkillDisplay>(
        "SELECT DISTINCT s.slug as skill_slug, sc.channel, b.slug as bundle_slug \
         FROM customer_bundles cb \
         JOIN bundle_items bi ON bi.bundle_id = cb.bundle_id \
         JOIN skill_channels sc ON sc.id = bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         JOIN bundles b ON b.id = cb.bundle_id \
         WHERE cb.customer_id = $1 \
         ORDER BY s.slug, sc.channel",
    )
    .bind(uuid)
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(rows)
}

#[server]
async fn list_all_skill_channels() -> Result<Vec<SkillChannelDisplay>, ServerFnError> {
    let pool = crate::server_pool()?;
    let channels = sqlx::query_as::<_, SkillChannelDisplay>(
        "SELECT sc.id, s.slug as skill_slug, sc.channel \
         FROM skill_channels sc \
         JOIN skills s ON s.id = sc.skill_id \
         ORDER BY s.slug, sc.channel",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(channels)
}

/// All bundles for the assignment dropdown.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct BundleOption {
    pub id: uuid::Uuid,
    pub slug: String,
    pub name: String,
}

#[server]
async fn list_all_bundles() -> Result<Vec<BundleOption>, ServerFnError> {
    let pool = crate::server_pool()?;
    let bundles = sqlx::query_as::<_, BundleOption>(
        "SELECT id, slug, name FROM bundles ORDER BY slug",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(bundles)
}

#[server]
async fn add_customer_skill(customer_id: String, skill_channel_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = customer_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let scid: uuid::Uuid = skill_channel_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("INSERT INTO customer_skills (customer_id, skill_channel_id) VALUES ($1, $2)")
        .bind(cid)
        .bind(scid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn remove_customer_skill(customer_skill_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = customer_skill_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM customer_skills WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn add_customer_bundle(customer_id: String, bundle_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let cid: uuid::Uuid = customer_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    let bid: uuid::Uuid = bundle_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;

    // Check for overlap: does the new bundle share any skill_channel_id with
    // any bundle already assigned to this customer?
    let overlap = sqlx::query_scalar::<_, String>(
        "SELECT s.slug || '/' || sc.channel \
         FROM bundle_items new_bi \
         JOIN bundle_items existing_bi ON existing_bi.skill_channel_id = new_bi.skill_channel_id \
         JOIN customer_bundles cb ON cb.bundle_id = existing_bi.bundle_id AND cb.customer_id = $1 \
         JOIN skill_channels sc ON sc.id = new_bi.skill_channel_id \
         JOIN skills s ON s.id = sc.skill_id \
         WHERE new_bi.bundle_id = $2 \
         LIMIT 1",
    )
    .bind(cid)
    .bind(bid)
    .fetch_optional(&pool)
    .await
    .map_err(|e| ServerFnError::new(e.to_string()))?;

    if let Some(conflicting) = overlap {
        return Err(ServerFnError::new(format!(
            "bundle conflicts with an already-assigned bundle on skill channel: {conflicting}"
        )));
    }

    sqlx::query("INSERT INTO customer_bundles (customer_id, bundle_id) VALUES ($1, $2)")
        .bind(cid)
        .bind(bid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[server]
async fn remove_customer_bundle(customer_bundle_id: String) -> Result<(), ServerFnError> {
    let pool = crate::server_pool()?;
    let uuid: uuid::Uuid = customer_bundle_id.parse().map_err(|e: uuid::Error| ServerFnError::new(e.to_string()))?;
    sqlx::query("DELETE FROM customer_bundles WHERE id = $1")
        .bind(uuid)
        .execute(&pool)
        .await
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

#[component]
pub fn CustomerSkills(customer_id: String) -> Element {
    let cid_skills = customer_id.clone();
    let mut skills = use_server_future(move || {
        let cid = cid_skills.clone();
        async move { list_customer_skills(cid).await }
    })?;

    let cid_bundles = customer_id.clone();
    let mut bundles = use_server_future(move || {
        let cid = cid_bundles.clone();
        async move { list_customer_bundles(cid).await }
    })?;

    let cid_bskills = customer_id.clone();
    let bundle_skills = use_server_future(move || {
        let cid = cid_bskills.clone();
        async move { list_bundle_skills(cid).await }
    })?;

    let available_sc = use_server_future(list_all_skill_channels)?;
    let available_bundles = use_server_future(list_all_bundles)?;

    let mut selected_sc = use_signal(String::new);
    let mut selected_bundle = use_signal(String::new);
    let mut bundle_error = use_signal(|| None::<String>);

    let cid_add_skill = customer_id.clone();
    let cid_add_bundle = customer_id.clone();

    rsx! {
        // Direct skill assignments
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-gray-700 mb-2", "Direct Skills" }
            form {
                class: "flex gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let cid = cid_add_skill.clone();
                    let scid = selected_sc.read().clone();
                    spawn(async move {
                        if !scid.is_empty() {
                            if add_customer_skill(cid, scid).await.is_ok() {
                                selected_sc.set(String::new());
                                skills.restart();
                            }
                        }
                    });
                },
                select {
                    class: "flex-1 border border-gray-300 rounded px-2 py-1 text-sm",
                    value: "{selected_sc}",
                    onchange: move |evt| selected_sc.set(evt.value()),
                    option { value: "", "Select skill/channel..." }
                    {match &*available_sc.read() {
                        Some(Ok(list)) => rsx! {
                            for sc in list {
                                {
                                    let val = sc.id.to_string();
                                    let label = format!("{} / {}", sc.skill_slug, sc.channel);
                                    rsx! { option { value: "{val}", "{label}" } }
                                }
                            }
                        },
                        _ => rsx! {},
                    }}
                }
                button {
                    class: "bg-blue-600 text-white px-2 py-1 rounded text-xs hover:bg-blue-700",
                    r#type: "submit",
                    "Add"
                }
            }
            {match &*skills.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-xs text-gray-400", "No direct skill assignments." }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200",
                        for cs in list {
                            {
                                let csid = cs.customer_skill_id.to_string();
                                let label = format!("{} / {}", cs.skill_slug, cs.channel);
                                rsx! {
                                    li { class: "py-1 flex justify-between items-center",
                                        span { class: "text-sm font-mono", "{label}" }
                                        button {
                                            class: "text-xs text-red-600 hover:underline",
                                            onclick: move |_| {
                                                let csid = csid.clone();
                                                spawn(async move {
                                                    if remove_customer_skill(csid).await.is_ok() {
                                                        skills.restart();
                                                    }
                                                });
                                            },
                                            "Remove"
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "text-red-600 text-xs", "Error: {e}" } },
                None => rsx! { p { class: "text-xs", "Loading..." } },
            }}
        }

        // Skills from bundles (read-only, green)
        div { class: "mb-4",
            h4 { class: "text-sm font-semibold text-green-700 mb-2", "From Bundles" }
            {match &*bundle_skills.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-xs text-gray-400", "No skills from bundles." }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200",
                        for bs in list {
                            {
                                let label = format!("{} / {}", bs.skill_slug, bs.channel);
                                let via = bs.bundle_slug.clone();
                                rsx! {
                                    li { class: "py-1 flex items-center gap-2",
                                        span { class: "text-sm font-mono text-green-700", "{label}" }
                                        span { class: "text-xs text-green-500", "via {via}" }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "text-red-600 text-xs", "Error: {e}" } },
                None => rsx! { p { class: "text-xs", "Loading..." } },
            }}
        }

        // Bundle assignments
        div {
            h4 { class: "text-sm font-semibold text-gray-700 mb-2", "Bundles" }
            if let Some(err) = &*bundle_error.read() {
                p { class: "text-red-600 text-xs mb-2", "{err}" }
            }
            form {
                class: "flex gap-2 mb-3",
                onsubmit: move |evt: FormEvent| {
                    evt.prevent_default();
                    let cid = cid_add_bundle.clone();
                    let bid = selected_bundle.read().clone();
                    spawn(async move {
                        if !bid.is_empty() {
                            match add_customer_bundle(cid, bid).await {
                                Ok(()) => {
                                    bundle_error.set(None);
                                    selected_bundle.set(String::new());
                                    bundles.restart();
                                }
                                Err(e) => {
                                    bundle_error.set(Some(e.to_string()));
                                }
                            }
                        }
                    });
                },
                select {
                    class: "flex-1 border border-gray-300 rounded px-2 py-1 text-sm",
                    value: "{selected_bundle}",
                    onchange: move |evt| selected_bundle.set(evt.value()),
                    option { value: "", "Select bundle..." }
                    {match &*available_bundles.read() {
                        Some(Ok(list)) => rsx! {
                            for b in list {
                                {
                                    let val = b.id.to_string();
                                    let label = format!("{} ({})", b.name, b.slug);
                                    rsx! { option { value: "{val}", "{label}" } }
                                }
                            }
                        },
                        _ => rsx! {},
                    }}
                }
                button {
                    class: "bg-blue-600 text-white px-2 py-1 rounded text-xs hover:bg-blue-700",
                    r#type: "submit",
                    "Add"
                }
            }
            {match &*bundles.read() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-xs text-gray-400", "No bundle assignments." }
                },
                Some(Ok(list)) => rsx! {
                    ul { class: "divide-y divide-gray-200",
                        for cb in list {
                            {
                                let cbid = cb.customer_bundle_id.to_string();
                                let label = format!("{} ({})", cb.bundle_name, cb.bundle_slug);
                                rsx! {
                                    li { class: "py-1 flex justify-between items-center",
                                        span { class: "text-sm", "{label}" }
                                        button {
                                            class: "text-xs text-red-600 hover:underline",
                                            onclick: move |_| {
                                                let cbid = cbid.clone();
                                                spawn(async move {
                                                    if remove_customer_bundle(cbid).await.is_ok() {
                                                        bundles.restart();
                                                    }
                                                });
                                            },
                                            "Remove"
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "text-red-600 text-xs", "Error: {e}" } },
                None => rsx! { p { class: "text-xs", "Loading..." } },
            }}
        }
    }
}
