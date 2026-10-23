use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_cli_help_and_version() {
    let mut cmd = Command::cargo_bin("gdrive").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Google Drive CLI & MCP Server"));

    let mut cmd_ver = Command::cargo_bin("gdrive").unwrap();
    cmd_ver
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("gdrive"));
}

#[test]
fn test_cli_auth_status_empty() {
    let temp_dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("gdrive").unwrap();
    cmd.env("GDRIVE_CONFIG_DIR", temp_dir.path())
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Belum terautentikasi"));
}

#[test]
fn test_cli_auth_profile_lifecycle() {
    let temp_dir = tempdir().unwrap();

    // 1. List profiles initially
    let mut cmd_list = Command::cargo_bin("gdrive").unwrap();
    cmd_list
        .env("GDRIVE_CONFIG_DIR", temp_dir.path())
        .args(["auth", "profile", "list"])
        .assert()
        .success();

    // 2. Switch to a new profile 'testprof'
    let mut cmd_switch = Command::cargo_bin("gdrive").unwrap();
    cmd_switch
        .env("GDRIVE_CONFIG_DIR", temp_dir.path())
        .args(["auth", "profile", "switch", "testprof"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Active profile: testprof"));

    // Verify status reflects active profile 'testprof'
    let mut cmd_status = Command::cargo_bin("gdrive").unwrap();
    cmd_status
        .env("GDRIVE_CONFIG_DIR", temp_dir.path())
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Profil: 'testprof'"));

    // 3. Remove the profile
    let mut cmd_remove = Command::cargo_bin("gdrive").unwrap();
    cmd_remove
        .env("GDRIVE_CONFIG_DIR", temp_dir.path())
        .args(["auth", "profile", "remove", "testprof"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Removed profile: testprof"));
}

#[test]
fn test_cli_setup_wizard_piped() {
    let temp_dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("gdrive").unwrap();

    let input = "mock_client_id_12345\nmock_client_secret_67890\n";
    cmd.env("GDRIVE_CONFIG_DIR", temp_dir.path())
        .args(["auth", "setup", "--skip-login"])
        .write_stdin(input)
        .assert()
        .success();

    let creds_file = temp_dir.path().join("credentials.json");
    assert!(creds_file.exists(), "credentials.json should be created");

    let content = fs::read_to_string(&creds_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(
        json["installed"]["client_id"], "mock_client_id_12345",
        "client_id must match piped input"
    );
    assert_eq!(
        json["installed"]["client_secret"], "mock_client_secret_67890",
        "client_secret must match piped input"
    );
}

#[test]
fn test_cli_unauthenticated_command_hint() {
    let temp_dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("gdrive").unwrap();

    cmd.env("GDRIVE_CONFIG_DIR", temp_dir.path())
        .args(["search", "test"])
        .assert()
        .failure();
}

#[test]
fn test_cli_doctor() {
    let temp_dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("gdrive").unwrap();

    cmd.env("GDRIVE_CONFIG_DIR", temp_dir.path())
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicate::str::contains("Google Drive MCP & CLI Doctor"))
        .stdout(predicate::str::contains("Config Directory"))
        .stdout(predicate::str::contains("Hasil Pemeriksaan"));
}
