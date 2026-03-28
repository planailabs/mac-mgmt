use std::sync::LazyLock;

/// The mcpServers entry sub-schema extracted from mcporter's schema.
/// Source: https://raw.githubusercontent.com/steipete/mcporter/main/mcporter.schema.json
/// (the `additionalProperties` value under `mcpServers`)
const MCP_SERVER_SCHEMA_JSON: &str = r#"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "description": {
      "description": "Human-readable description of the server",
      "type": "string"
    },
    "baseUrl": {
      "description": "Base URL for HTTP/SSE transport (camelCase)",
      "type": "string"
    },
    "base_url": {
      "description": "Base URL for HTTP/SSE transport (snake_case)",
      "type": "string"
    },
    "url": {
      "description": "Server URL for HTTP/SSE transport",
      "type": "string"
    },
    "serverUrl": {
      "description": "Server URL for HTTP/SSE transport (camelCase)",
      "type": "string"
    },
    "server_url": {
      "description": "Server URL for HTTP/SSE transport (snake_case)",
      "type": "string"
    },
    "command": {
      "description": "Command to spawn for stdio transport (string or array of arguments)",
      "anyOf": [
        { "type": "string" },
        { "type": "array", "items": { "type": "string" } }
      ]
    },
    "executable": {
      "description": "Executable path for stdio transport",
      "type": "string"
    },
    "args": {
      "description": "Arguments to pass to the stdio command",
      "type": "array",
      "items": { "type": "string" }
    },
    "headers": {
      "description": "HTTP headers for requests",
      "type": "object",
      "propertyNames": { "type": "string" },
      "additionalProperties": { "type": "string" }
    },
    "env": {
      "description": "Environment variables for stdio commands",
      "type": "object",
      "propertyNames": { "type": "string" },
      "additionalProperties": { "type": "string" }
    },
    "auth": {
      "description": "Authentication method (e.g., \"oauth\")",
      "type": "string"
    },
    "tokenCacheDir": { "type": "string" },
    "token_cache_dir": { "type": "string" },
    "clientName": { "type": "string" },
    "client_name": { "type": "string" },
    "oauthRedirectUrl": { "type": "string" },
    "oauth_redirect_url": { "type": "string" },
    "oauthScope": { "type": "string" },
    "oauth_scope": { "type": "string" },
    "oauthCommand": {
      "type": "object",
      "properties": {
        "args": { "type": "array", "items": { "type": "string" } }
      },
      "required": ["args"],
      "additionalProperties": false
    },
    "oauth_command": {
      "type": "object",
      "properties": {
        "args": { "type": "array", "items": { "type": "string" } }
      },
      "required": ["args"],
      "additionalProperties": false
    },
    "bearerToken": { "type": "string" },
    "bearer_token": { "type": "string" },
    "bearerTokenEnv": { "type": "string" },
    "bearer_token_env": { "type": "string" },
    "lifecycle": {
      "anyOf": [
        { "type": "string", "const": "keep-alive" },
        { "type": "string", "const": "ephemeral" },
        {
          "type": "object",
          "properties": {
            "mode": {
              "anyOf": [
                { "type": "string", "const": "keep-alive" },
                { "type": "string", "const": "ephemeral" }
              ]
            },
            "idleTimeoutMs": {
              "type": "integer",
              "exclusiveMinimum": 0,
              "maximum": 9007199254740991
            }
          },
          "required": ["mode"],
          "additionalProperties": false
        }
      ]
    },
    "logging": {
      "type": "object",
      "properties": {
        "daemon": {
          "type": "object",
          "properties": {
            "enabled": { "type": "boolean" }
          },
          "additionalProperties": false
        }
      },
      "additionalProperties": false
    }
  },
  "additionalProperties": false
}"#;

static VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
    let schema: serde_json::Value =
        serde_json::from_str(MCP_SERVER_SCHEMA_JSON).expect("embedded MCP schema is invalid JSON");
    jsonschema::validator_for(&schema).expect("embedded MCP schema is invalid JSON Schema")
});

/// Validate a `config_json` value against the mcpServers entry schema.
/// Returns `Ok(())` if valid, or `Err(message)` with all validation errors joined.
pub fn validate_mcp_server_config(config: &serde_json::Value) -> Result<(), String> {
    let errors: Vec<String> = VALIDATOR
        .iter_errors(config)
        .map(|e| e.to_string())
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}
