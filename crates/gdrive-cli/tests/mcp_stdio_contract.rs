use assert_cmd::cargo::cargo_bin;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn test_mcp_stdio_jsonrpc_contract() {
    let bin_path = cargo_bin("gdrive");
    let mut child = Command::new(bin_path)
        .args(["mcp", "serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to spawn gdrive mcp serve");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");
    let mut reader = BufReader::new(stdout);

    // 1. Send initialize request
    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "contract-test-runner",
                "version": "1.0.0"
            }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init_req).unwrap()).unwrap();
    stdin.flush().unwrap();

    let mut init_resp_line = String::new();
    reader
        .read_line(&mut init_resp_line)
        .expect("Failed to read initialize response");

    // Rule 1: Clean stdout - must parse as valid JSON-RPC 2.0
    assert!(
        !init_resp_line.trim().is_empty(),
        "initialize response line must not be empty"
    );
    let init_json: Value = serde_json::from_str(&init_resp_line)
        .expect("Initialize response must be valid JSON-RPC 2.0 without stray stdout logs");

    assert_eq!(init_json["jsonrpc"], "2.0");
    assert_eq!(init_json["id"], 1);
    assert_eq!(init_json["result"]["serverInfo"]["name"], "gdrive-mcp");
    assert!(init_json["result"]["capabilities"]["tools"].is_object());

    // 2. Send notifications/initialized
    let notif = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    writeln!(stdin, "{}", serde_json::to_string(&notif).unwrap()).unwrap();
    stdin.flush().unwrap();

    // 3. Send tools/list request
    let tools_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });
    writeln!(stdin, "{}", serde_json::to_string(&tools_req).unwrap()).unwrap();
    stdin.flush().unwrap();

    let mut tools_resp_line = String::new();
    reader
        .read_line(&mut tools_resp_line)
        .expect("Failed to read tools/list response");

    // Rule 1: Clean stdout
    let tools_json: Value = serde_json::from_str(&tools_resp_line)
        .expect("Tools list response must be valid JSON-RPC 2.0 without stray stdout logs");

    assert_eq!(tools_json["jsonrpc"], "2.0");
    assert_eq!(tools_json["id"], 2);

    let tools_array = tools_json["result"]["tools"]
        .as_array()
        .expect("tools field must be an array");
    assert!(
        !tools_array.is_empty(),
        "Tool router must return registered tools"
    );

    let tool_names: Vec<&str> = tools_array
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    // Required core tools
    let expected_tools = [
        "gdrive_search",
        "gdrive_read_file",
        "gdrive_create_file",
        "gdrive_update_file",
        "gdrive_download_file",
        "gdrive_trash_file",
        "gdrive_create_folder",
        "gdrive_rename_file",
        "gdrive_move_file",
        "gdrive_list_drives",
        "gdrive_list_folder",
        "gdrive_doctor",
    ];
    for expected in expected_tools {
        assert!(
            tool_names.contains(&expected),
            "Expected tool '{}' was not found in registered tools list: {:?}",
            expected,
            tool_names
        );
    }

    // Rule 2: Strict schema safety - no raw boolean schemas in any tool inputSchema properties
    for tool in tools_array {
        let name = tool["name"].as_str().unwrap_or("unknown");
        let input_schema = &tool["inputSchema"];
        assert!(
            input_schema.is_object(),
            "Tool '{}' inputSchema must be an object schema",
            name
        );

        if let Some(properties) = input_schema.get("properties").and_then(|p| p.as_object()) {
            for (prop_name, prop_val) in properties {
                assert!(
                    !prop_val.is_boolean(),
                    "Tool '{}' property '{}' has a raw boolean schema: {:?}. Rule 2 violation!",
                    name,
                    prop_name,
                    prop_val
                );
                assert!(
                    prop_val.is_object(),
                    "Tool '{}' property '{}' must be an object schema with type and description: {:?}",
                    name,
                    prop_name,
                    prop_val
                );
            }
        }
    }

    // Close stdin and wait for child
    drop(stdin);
    let _ = child.wait();
}
