use cloudflare_api::Client;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn init_tls() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[tokio::test]
async fn test_list_zones() {
    init_tls();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/zones"))
        .and(header("Authorization", "Bearer test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "errors": [],
            "messages": [],
            "result": [{
                "id": "zone-abc123",
                "name": "example.com",
                "status": "active",
                "type": "full",
                "name_servers": ["ns1.cloudflare.com", "ns2.cloudflare.com"],
                "account": { "id": "acct-1", "name": "Test Account" }
            }],
            "result_info": { "page": 1, "per_page": 50, "count": 1, "total_count": 1 }
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(&server.uri(), "test-token");
    let zones = client.list_zones(None).await.unwrap();
    assert_eq!(zones.len(), 1);
    assert_eq!(zones[0].name, "example.com");
    assert_eq!(zones[0].id, "zone-abc123");
    assert_eq!(zones[0].status, "active");
}

#[tokio::test]
async fn test_create_zone() {
    init_tls();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/zones"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "errors": [],
            "messages": [],
            "result": {
                "id": "zone-new",
                "name": "newdomain.com",
                "status": "pending",
                "name_servers": ["ns1.cloudflare.com", "ns2.cloudflare.com"]
            }
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(&server.uri(), "test-token");
    let zone = client.create_zone("newdomain.com", "acct-1").await.unwrap();
    assert_eq!(zone.name, "newdomain.com");
    assert_eq!(zone.status, "pending");
}

#[tokio::test]
async fn test_list_dns_records() {
    init_tls();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/zones/zone-1/dns_records"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "errors": [],
            "messages": [],
            "result": [
                {
                    "id": "rec-1",
                    "type": "A",
                    "name": "example.com",
                    "content": "1.2.3.4",
                    "ttl": 1,
                    "proxied": true
                },
                {
                    "id": "rec-2",
                    "type": "CNAME",
                    "name": "www.example.com",
                    "content": "example.com",
                    "ttl": 300,
                    "proxied": true
                }
            ]
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(&server.uri(), "test-token");
    let records = client.list_dns_records("zone-1").await.unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].record_type, "A");
    assert_eq!(records[0].content.as_deref(), Some("1.2.3.4"));
}

#[tokio::test]
async fn test_api_error() {
    init_tls();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/zones"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "success": false,
            "errors": [{ "code": 9109, "message": "Invalid access token" }],
            "messages": [],
            "result": null
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(&server.uri(), "bad-token");
    let result = client.list_zones(None).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        cloudflare_api::Error::Api(errors) => {
            assert_eq!(errors[0].code, 9109);
        }
        e => panic!("expected Api error, got: {e:?}"),
    }
}

#[tokio::test]
async fn test_check_domains() {
    init_tls();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/accounts/acct-1/registrar/domain-check"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "success": true,
            "errors": [],
            "messages": [],
            "result": [
                {
                    "name": "example.com",
                    "registrable": false,
                    "reason": "domain_unavailable"
                },
                {
                    "name": "available-domain.dev",
                    "registrable": true,
                    "tier": "standard",
                    "pricing": {
                        "currency": "USD",
                        "registration_cost": "12.00",
                        "renewal_cost": "12.00"
                    }
                }
            ]
        })))
        .mount(&server)
        .await;

    let client = Client::with_base_url(&server.uri(), "test-token");
    let domains = vec!["example.com".into(), "available-domain.dev".into()];
    let checks = client.check_domains("acct-1", &domains).await.unwrap();
    assert_eq!(checks.len(), 2);
    assert!(!checks[0].registrable);
    assert!(checks[1].registrable);
    assert_eq!(checks[1].pricing.as_ref().unwrap().registration_cost.as_deref(), Some("12.00"));
}
