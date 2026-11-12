use assert_cmd::Command;
use predicates::str::contains;

const SAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fsim-blueprint/tests/sample-blueprints/example_1"
);

fn fsim() -> Command {
    Command::cargo_bin("fsim").unwrap()
}

#[test]
fn decode_prints_a_summary() {
    fsim()
        .args(["decode", SAMPLE])
        .assert()
        .success()
        .stdout(contains("236 entities"))
        .stdout(contains("chemical-plant"))
        .stdout(contains("efficiency-module-2"));
}

#[test]
fn decode_reads_stdin() {
    let text = std::fs::read_to_string(SAMPLE).unwrap();
    fsim()
        .args(["decode", "-"])
        .write_stdin(text)
        .assert()
        .success()
        .stdout(contains("236 entities"));
}

#[test]
fn decode_json_is_valid_json() {
    let output = fsim().args(["decode", "--json", SAMPLE]).output().unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["blueprint"]["entities"].is_array());
}

#[test]
fn decode_rejects_garbage_without_panicking() {
    fsim()
        .args(["decode", "-"])
        .write_stdin("1garbage")
        .assert()
        .failure()
        .stderr(contains("unsupported format version"));
}

#[test]
fn inspect_reports_a_missing_dump() {
    fsim()
        .args([
            "inspect",
            "recipe",
            "iron-gear-wheel",
            "--data",
            "/nonexistent",
        ])
        .assert()
        .failure()
        .stderr(contains("not found"));
}

#[test]
fn calc_reports_a_missing_dump() {
    fsim()
        .args(["calc", SAMPLE, "--data", "/nonexistent"])
        .assert()
        .failure()
        .stderr(contains("not found"));
}
