use serde_json::json;
use spaceship_api::Client;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn init_tls() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[tokio::test]
async fn test_list_domains() {
    init_tls();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/domains"))
        .and(header("X-Api-Key", "test-key"))
        .and(header("X-Api-Secret", "test-secret"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [
                {
                    "name": "example.com",
                    "autoRenew": true,
                    "registrationDate": "2024-01-15T10:00:00Z",
                    "expirationDate": "2025-01-15T10:00:00Z",
                    "lifecycleStatus": "registered"
                }
            ],
            "totalCount": 1
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(&server.uri(), "test-key", "test-secret");
    let resp = client.list_domains(0, 100).await.unwrap();
    assert_eq!(resp.items.len(), 1);
    assert_eq!(resp.items[0].name, "example.com");
    assert_eq!(resp.items[0].lifecycle_status.as_deref(), Some("registered"));
}

#[tokio::test]
async fn test_check_availability() {
    init_tls();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/domains/available-domain.dev/available"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "domain": "available-domain.dev",
            "status": "available",
            "premiumPricing": [
                { "operation": "register", "price": 10.99, "currency": "USD" },
                { "operation": "renew", "price": 10.99, "currency": "USD" }
            ]
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(&server.uri(), "test-key", "test-secret");
    let avail = client.check_availability("available-domain.dev").await.unwrap();
    assert_eq!(avail.status.as_deref(), Some("available"));
    assert_eq!(avail.premium_pricing.len(), 2);
    assert_eq!(avail.premium_pricing[0].price, 10.99);
}

#[tokio::test]
async fn test_save_contact() {
    init_tls();
    let server = MockServer::start().await;

    Mock::given(method("PUT"))
        .and(path("/v1/contacts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "contactId": "contact-abc123"
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(&server.uri(), "test-key", "test-secret");
    let contact = spaceship_api::ContactDetails {
        first_name: "John".into(),
        last_name: "Doe".into(),
        email: "john@example.com".into(),
        phone: "+1234567890".into(),
        address1: "123 Main St".into(),
        city: "New York".into(),
        country: "US".into(),
        organization: None,
        address2: None,
        state_province: Some("NY".into()),
        postal_code: Some("10001".into()),
    };
    let resp = client.save_contact(&contact).await.unwrap();
    assert_eq!(resp.contact_id, "contact-abc123");
}

#[tokio::test]
async fn test_api_error() {
    init_tls();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/domains"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "detail": "Invalid API credentials"
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(&server.uri(), "bad-key", "bad-secret");
    let result = client.list_domains(0, 10).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        spaceship_api::Error::Api { status, .. } => assert_eq!(status, 401),
        e => panic!("expected Api error, got: {e:?}"),
    }
}

#[tokio::test]
async fn test_async_operation_success() {
    init_tls();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/async-operations/op-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "success",
            "type": "domains_Create",
            "details": { "domain": "new-domain.com" },
            "createdAt": "2024-01-15T10:00:00Z",
            "modifiedAt": "2024-01-15T10:01:00Z"
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(&server.uri(), "test-key", "test-secret");
    let op = client.get_async_operation("op-123").await.unwrap();
    assert_eq!(op.status, "success");
    assert_eq!(op.op_type.as_deref(), Some("domains_Create"));
}
