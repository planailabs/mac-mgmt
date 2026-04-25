//! Integration tests for mac-mgmt-executor MCP server.
//!
//! These tests spawn the binary as a child process and communicate with it
//! over stdio using the MCP protocol via rmcp's client.
//!
//! Requires `incus` CLI available and working on the host.

use rmcp::model::CallToolRequestParams;
use rmcp::transport::TokioChildProcess;
use rmcp::ServiceExt;
use serde_json::json;

fn build_child() -> TokioChildProcess {
    let cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_mac-mgmt-executor"));
    TokioChildProcess::new(cmd).expect("failed to spawn executor")
}

async fn connect() -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    ().serve(build_child())
        .await
        .expect("MCP handshake failed")
}

fn call(name: &str, args: serde_json::Value) -> CallToolRequestParams {
    CallToolRequestParams {
        meta: None,
        name: name.to_string().into(),
        arguments: args.as_object().cloned(),
        task: None,
    }
}

fn result_text(result: &rmcp::model::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.as_str()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn test_list_tools() {
    let client = connect().await;
    let tools = client.list_tools(Default::default()).await.unwrap();
    let names: Vec<&str> = tools.tools.iter().map(|t| t.name.as_ref()).collect();
    assert!(names.contains(&"os_list"), "missing os_list tool");
    assert!(names.contains(&"system_create"), "missing system_create tool");
    assert!(names.contains(&"system_execute"), "missing system_execute tool");
    assert!(names.contains(&"system_destroy"), "missing system_destroy tool");
    assert!(names.contains(&"system_list"), "missing system_list tool");
    assert!(names.contains(&"system_file_write"), "missing system_file_write tool");
    assert!(names.contains(&"system_file_read"), "missing system_file_read tool");
    assert_eq!(names.len(), 7);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn test_os_list() {
    let client = connect().await;
    let result = client.call_tool(call("os_list", json!({}))).await.unwrap();
    let text = result_text(&result);
    assert!(
        text.contains("ubuntu") || text.contains("alpine"),
        "os_list should return images: {text}"
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn test_os_list_with_filter() {
    let client = connect().await;
    let result = client
        .call_tool(call("os_list", json!({"filter": "alpine"})))
        .await
        .unwrap();
    let text = result_text(&result);
    assert!(
        text.contains("alpine"),
        "filtered list should contain alpine: {text}"
    );
    assert!(
        !text.contains("ubuntu/"),
        "filtered list should not contain ubuntu: {text}"
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn test_system_execute_without_container() {
    let client = connect().await;
    let result = client
        .call_tool(call("system_execute", json!({"command": "echo hello"})))
        .await
        .unwrap();
    let text = result_text(&result);
    assert!(
        text.contains("no container created yet"),
        "should error without container: {text}"
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn test_system_list_empty() {
    let client = connect().await;
    let result = client
        .call_tool(call("system_list", json!({})))
        .await
        .unwrap();
    let text = result_text(&result);
    assert!(
        text.contains("No containers"),
        "empty list message expected: {text}"
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn test_system_destroy_nonexistent() {
    let client = connect().await;
    let result = client
        .call_tool(call("system_destroy", json!({"name": "does-not-exist"})))
        .await
        .unwrap();
    let text = result_text(&result);
    assert!(
        text.contains("No container named"),
        "should report not found: {text}"
    );
    client.cancel().await.unwrap();
}

/// Full lifecycle test: create -> execute -> file_write -> file_read -> list -> destroy.
/// This test is slow (~15-30s) as it launches a real container.
#[tokio::test]
async fn test_full_lifecycle() {
    let client = connect().await;

    // Create a container.
    let result = client
        .call_tool(call("system_create", json!({"os": "alpine/3.21"})))
        .await
        .unwrap();
    let text = result_text(&result);
    assert!(
        text.contains("created and running"),
        "create should succeed: {text}"
    );
    // Extract the container name from "Container '<name>' created and running"
    let container_name = text
        .split('\'')
        .nth(1)
        .expect("couldn't extract container name from create response")
        .to_string();

    // Execute a command.
    let result = client
        .call_tool(call("system_execute", json!({"command": "cat /etc/os-release"})))
        .await
        .unwrap();
    let text = result_text(&result);
    assert!(
        text.contains("Alpine"),
        "should see Alpine in os-release: {text}"
    );

    // Write a file.
    let result = client
        .call_tool(call(
            "system_file_write",
            json!({"path": "/tmp/test.txt", "content": "hello from test"}),
        ))
        .await
        .unwrap();
    let text = result_text(&result);
    assert!(
        text.contains("File written"),
        "file write should succeed: {text}"
    );

    // Read the file back.
    let result = client
        .call_tool(call("system_file_read", json!({"path": "/tmp/test.txt"})))
        .await
        .unwrap();
    let text = result_text(&result);
    assert_eq!(text.trim(), "hello from test", "file content should match");

    // List containers.
    let result = client
        .call_tool(call("system_list", json!({})))
        .await
        .unwrap();
    let text = result_text(&result);
    assert!(
        text.contains(&container_name),
        "list should include our container: {text}"
    );
    assert!(
        text.contains("alpine/3.21"),
        "list should show image: {text}"
    );

    // Execute with non-zero exit code.
    let result = client
        .call_tool(call("system_execute", json!({"command": "exit 42"})))
        .await
        .unwrap();
    let text = result_text(&result);
    assert!(
        text.contains("exit code: 42"),
        "should report exit code: {text}"
    );

    // Destroy the container.
    let result = client
        .call_tool(call("system_destroy", json!({"name": container_name})))
        .await
        .unwrap();
    let text = result_text(&result);
    assert!(
        text.contains("destroyed"),
        "destroy should succeed: {text}"
    );

    // Verify it's gone from the list.
    let result = client
        .call_tool(call("system_list", json!({})))
        .await
        .unwrap();
    let text = result_text(&result);
    assert!(
        text.contains("No containers"),
        "list should be empty after destroy: {text}"
    );

    client.cancel().await.unwrap();
}
