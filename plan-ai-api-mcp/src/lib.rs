//! `plan-ai-api-mcp` — declare a CRUD operation once, serve it as a REST HTTP
//! route, an MCP tool, and an OpenAPI operation, sharing one handler, one input
//! schema, and one authorization model.
//!
//! ```ignore
//! let mut reg = Registry::new(auth).info("Web Agency API", "1.0");
//! {
//!     let mut d = reg.resource("domains", "domain", "Domains");
//!     d.list(  "List domains",   |st, p, i: DomainListInput|   async move { /* ... */ });
//!     d.get(   "Get a domain",   |st, p, i: DomainGetInput|    async move { /* ... */ });
//!     d.create("Create a domain",|st, p, i: DomainCreateInput| async move { /* ... */ });
//! }
//! let http = reg.http_router(state.clone());   // /api/v1/domains, /api/v1/openapi.json, /api/v1/docs
//! let mcp  = reg.mcp_service(state);           // mount at /mcp
//! ```

pub mod auth;
pub mod error;
pub mod principal;
pub mod registry;
pub mod server;

pub use auth::{Authenticator, bearer_from_header};
pub use error::ApiError;
pub use principal::{OrgSet, Principal};
pub use registry::{Action, ErasedEndpoint, OnItem, Registry, ResourceBuilder, Risk};
pub use server::ApiMcpServer;
