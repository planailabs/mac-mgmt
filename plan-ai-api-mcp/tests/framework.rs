//! Framework behavior tests: no DB, a stub authenticator, an in-memory resource.

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use plan_ai_api_mcp::{ApiError, Authenticator, OnItem, Principal, Registry, Risk};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tower::ServiceExt; // oneshot
use uuid::Uuid;

const ORG_A: Uuid = Uuid::from_u128(0xa);
const ORG_B: Uuid = Uuid::from_u128(0xb);

// ── Stub auth ─────────────────────────────────────────────────────────

struct StubAuth;

#[async_trait]
impl Authenticator for StubAuth {
    async fn authenticate(&self, bearer: &str) -> Result<Principal, ApiError> {
        match bearer {
            "admin-token" => Ok(Principal::admin("admin")),
            "org-a-token" => Ok(Principal::scoped(
                "org-a",
                HashSet::from([ORG_A]),
                HashSet::from([ORG_A]),
            )),
            _ => Err(ApiError::unauthorized("bad token")),
        }
    }
}

// ── Domain types ──────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, JsonSchema, Clone)]
struct Widget {
    id: String,
    name: String,
}

#[derive(Deserialize, JsonSchema)]
struct ListInput {
    #[serde(default)]
    limit: Option<i64>,
}

#[derive(Deserialize, JsonSchema)]
struct GetInput {
    id: String,
}

#[derive(Deserialize, JsonSchema)]
struct CreateInput {
    name: String,
    org: Uuid,
}

fn build_registry() -> Registry<()> {
    let mut reg = Registry::new(Arc::new(StubAuth)).info("Test API", "1.0");
    {
        let mut w = reg.resource("widgets", "widget", "Widgets");
        w.list("List widgets", |_s, _p, i: ListInput| async move {
            let n = i.limit.unwrap_or(2).max(0) as usize;
            Ok((0..n)
                .map(|k| Widget {
                    id: k.to_string(),
                    name: format!("w{k}"),
                })
                .collect::<Vec<_>>())
        });
        w.get("Get widget", |_s, _p, i: GetInput| async move {
            Ok(Widget {
                id: i.id,
                name: "found".into(),
            })
        });
        w.create(
            "Create widget",
            |_s, p: Arc<Principal>, i: CreateInput| async move {
                p.require_write(&i.org)?;
                Ok(Widget {
                    id: "new".into(),
                    name: i.name,
                })
            },
        );
        w.custom(
            "wipe",
            Risk::Destructive,
            OnItem::No,
            "Admin-only wipe",
            |_s, p: Arc<Principal>, _i: ListInput| async move {
                p.require_admin()?;
                Ok(Widget {
                    id: "-".into(),
                    name: "wiped".into(),
                })
            },
        );
    }
    reg
}

async fn send(router: &axum::Router, req: Request<Body>) -> (StatusCode, String) {
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 1 << 20).await.unwrap();
    (status, String::from_utf8_lossy(&bytes).to_string())
}

fn get(path: &str, token: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method("GET").uri(path);
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

fn post(path: &str, token: Option<&str>, body: &str) -> Request<Body> {
    let mut b = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn missing_token_is_unauthorized() {
    let router = build_registry().http_router(());
    let (status, _) = send(&router, get("/api/v1/widgets", None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn bad_token_is_unauthorized() {
    let router = build_registry().http_router(());
    let (status, _) = send(&router, get("/api/v1/widgets", Some("nope"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_lists_widgets() {
    let router = build_registry().http_router(());
    let (status, body) = send(&router, get("/api/v1/widgets", Some("admin-token"))).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn get_alias_reads_query_param() {
    let router = build_registry().http_router(());
    let (status, body) = send(&router, get("/api/v1/widgets?limit=5", Some("admin-token"))).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 5);
}

#[tokio::test]
async fn item_get_merges_path_id() {
    let router = build_registry().http_router(());
    let (status, body) = send(&router, get("/api/v1/widgets/abc", Some("admin-token"))).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["id"], "abc");
}

#[tokio::test]
async fn org_scoped_write_is_enforced() {
    let router = build_registry().http_router(());

    // org-a token writing to org A → allowed
    let (status, _) = send(
        &router,
        post(
            "/api/v1/widgets",
            Some("org-a-token"),
            &format!(r#"{{"name":"x","org":"{ORG_A}"}}"#),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // org-a token writing to org B → forbidden
    let (status, _) = send(
        &router,
        post(
            "/api/v1/widgets",
            Some("org-a-token"),
            &format!(r#"{{"name":"x","org":"{ORG_B}"}}"#),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn malformed_body_is_bad_request() {
    let router = build_registry().http_router(());
    let (status, _) = send(
        &router,
        post("/api/v1/widgets", Some("admin-token"), "{ not json"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn custom_admin_only() {
    let router = build_registry().http_router(());
    let (status, _) = send(
        &router,
        post("/api/v1/widgets/wipe", Some("org-a-token"), "{}"),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = send(
        &router,
        post("/api/v1/widgets/wipe", Some("admin-token"), "{}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn tool_names_follow_convention() {
    let reg = build_registry();
    let names = reg.tool_names();
    assert!(names.contains(&"widget_list".to_string()));
    assert!(names.contains(&"widget_get".to_string()));
    assert!(names.contains(&"widget_create".to_string()));
    assert!(names.contains(&"widget_wipe".to_string()));
}

#[tokio::test]
async fn openapi_has_paths_and_security() {
    let reg = build_registry();
    let doc = reg.openapi_json();
    assert_eq!(doc["openapi"], "3.1.0");
    assert!(doc["paths"]["/api/v1/widgets"]["get"].is_object());
    assert!(doc["paths"]["/api/v1/widgets"]["post"].is_object());
    assert!(doc["paths"]["/api/v1/widgets/{id}"]["get"].is_object());
    assert_eq!(
        doc["components"]["securitySchemes"]["bearerAuth"]["scheme"],
        "bearer"
    );
    // path param present on item route
    let params = &doc["paths"]["/api/v1/widgets/{id}"]["get"]["parameters"];
    assert_eq!(params[0]["name"], "id");
    assert_eq!(params[0]["in"], "path");
}

#[tokio::test]
async fn openapi_parses_into_utoipa() {
    // The generated doc must round-trip into utoipa's typed OpenApi, which is
    // what http_router hands to Swagger UI.
    let reg = build_registry();
    let parsed: Result<utoipa::openapi::OpenApi, _> = serde_json::from_value(reg.openapi_json());
    assert!(parsed.is_ok(), "utoipa parse failed: {:?}", parsed.err());
}

#[tokio::test]
async fn openapi_json_served_by_utoipa() {
    let router = build_registry().http_router(());
    let (status, body) = send(&router, get("/api/v1/openapi.json", None)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("\"openapi\""));
    assert!(body.contains("/api/v1/widgets"));
}
