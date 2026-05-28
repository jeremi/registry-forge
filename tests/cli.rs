use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use zip::write::SimpleFileOptions;

fn bin() -> Command {
    Command::cargo_bin("registry-forge").expect("registry-forge binary")
}

fn fixture_project() -> (TempDir, PathBuf) {
    fixture_project_named("demo")
}

fn fixture_project_named(name: &str) -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().expect("tempdir");
    let project = temp.path().join(name);
    copy_dir(&Path::new("fixtures").join(name), &project);
    (temp, project)
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create dir");
    for entry in fs::read_dir(from).expect("read dir") {
        let entry = entry.expect("entry");
        let dest = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir(&entry.path(), &dest);
        } else {
            fs::copy(entry.path(), dest).expect("copy file");
        }
    }
}

fn run(project: &Path, args: &[&str]) {
    bin().current_dir(project).args(args).assert().success();
}

fn output(project: &Path, args: &[&str]) -> String {
    let output = bin()
        .current_dir(project)
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(output).expect("utf8 stdout")
}

fn recipe_arg() -> &'static str {
    "forge.recipe.yaml"
}

#[test]
fn full_demo_flow_exports_without_raw_source_and_replays() {
    let (_temp, project) = fixture_project();

    run(&project, &["check-recipe", recipe_arg()]);
    run(&project, &["inspect-source", recipe_arg()]);
    run(&project, &["profile-source", recipe_arg()]);
    run(&project, &["suggest-alignments", recipe_arg()]);
    run(&project, &["preview-transform", recipe_arg()]);
    run(
        &project,
        &[
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ],
    );
    run(
        &project,
        &[
            "export-package",
            recipe_arg(),
            "--out",
            "target/forge-demo-package",
        ],
    );
    run(
        &project,
        &[
            "check-recipe",
            "target/forge-demo-package/forge.recipe.yaml",
        ],
    );
    run(
        &project,
        &[
            "preview-transform",
            "--source-override",
            "data/farmers.csv",
            "--out",
            "target/replay-canonical-samples.redacted.jsonl",
            "target/forge-demo-package/forge.recipe.yaml",
        ],
    );
    run(
        &project,
        &[
            "validate-output",
            "--require-status",
            "ready_candidate",
            "--source-override",
            "data/farmers.csv",
            "target/forge-demo-package/forge.recipe.yaml",
        ],
    );

    let package = project.join("target/forge-demo-package");
    assert!(!package.join("data/farmers.csv").exists());
    assert!(!package.join("data/farmers.xlsx").exists());
    assert!(package.join("candidates/relay/candidate.json").exists());
    assert_eq!(
        fs::read(package.join("previews/canonical-samples.redacted.jsonl")).unwrap(),
        fs::read(project.join("target/replay-canonical-samples.redacted.jsonl")).unwrap()
    );
    let preview =
        fs::read_to_string(package.join("previews/canonical-samples.redacted.jsonl")).unwrap();
    assert!(preview.contains("[redacted]"));
    assert!(!preview.contains("Ana Demo"));

    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(package.join("package-manifest.json")).unwrap())
            .unwrap();
    let exported_recipe: Value =
        serde_yaml::from_str(&fs::read_to_string(package.join("forge.recipe.yaml")).unwrap())
            .unwrap();
    assert_eq!(
        exported_recipe["profile_bundle"]["path"],
        "profile-bundles/publicschema-demo.bundle.json"
    );
    assert_eq!(
        exported_recipe["mappings"]["file"],
        "mappings/crosswalk.mapping.yaml"
    );
    assert_eq!(
        manifest["source_hash"],
        "72e497effe7dc0c6de37789b607de153e8e27a39d54b23b61ed7c4e3f46f9fce"
    );
    assert_eq!(manifest["command_versions"]["registry-forge"], "0.1.0");
    let artifacts = manifest["artifacts"].as_object().unwrap();
    assert!(artifacts.contains_key("forge.recipe.yaml"));
    assert!(!artifacts.contains_key("package-manifest.json"));
    for (artifact, expected_hash) in artifacts {
        let bytes = fs::read(package.join(artifact)).unwrap();
        assert_eq!(sha256_hex(&bytes), expected_hash.as_str().unwrap());
    }
    assert_no_fixture_values(&package, &["Ana Demo", "Ben Demo", "Cara Demo"]);
}

#[test]
fn household_demo_flow_reaches_ready_candidate_and_redacts_names() {
    let (_temp, project) = fixture_project_named("demo-households-csv");

    run(&project, &["check-recipe", recipe_arg()]);
    run(&project, &["inspect-source", recipe_arg()]);
    run(&project, &["profile-source", recipe_arg()]);
    run(&project, &["suggest-alignments", recipe_arg()]);
    run(&project, &["preview-transform", recipe_arg()]);
    run(
        &project,
        &[
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ],
    );
    run(
        &project,
        &[
            "export-package",
            recipe_arg(),
            "--out",
            "target/household-package",
        ],
    );
    assert!(project
        .join("target/household-package/profile-bundles/publicschema-household.bundle.json")
        .exists());
    assert!(!project
        .join("target/household-package/profile-bundles/publicschema-demo.bundle.json")
        .exists());
    let exported_recipe: Value = serde_yaml::from_str(
        &fs::read_to_string(project.join("target/household-package/forge.recipe.yaml")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        exported_recipe["profile_bundle"]["path"],
        "profile-bundles/publicschema-household.bundle.json"
    );

    let profile: Value = serde_json::from_str(
        &fs::read_to_string(project.join("reports/source-profile.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(profile["row_count"], 4);
    assert!(profile["fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["name"] == "household_id" && field["candidate_identifier"] == true));

    let preview =
        fs::read_to_string(project.join("previews/canonical-samples.redacted.jsonl")).unwrap();
    assert!(preview.contains("[redacted]"));
    assert!(!preview.contains("Nora Example"));
}

#[test]
fn messy_csv_demo_reports_warnings_and_still_exports_redacted_package() {
    let (_temp, project) = fixture_project_named("demo-messy-csv");

    run(&project, &["check-recipe", recipe_arg()]);
    run(&project, &["inspect-source", recipe_arg()]);
    let inspection: Value = serde_json::from_str(
        &fs::read_to_string(project.join("reports/source-inspection.json")).unwrap(),
    )
    .unwrap();
    let warnings = inspection["warnings"].as_array().unwrap();
    assert!(inspection["stable_headers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "notes_2"));
    assert!(inspection["stable_headers"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value == "column_8"));
    assert!(warnings.iter().any(|value| value == "blank_header"));
    assert!(warnings.iter().any(|value| value
        .as_str()
        .unwrap_or_default()
        .starts_with("duplicate_header:Notes")));
    assert!(warnings
        .iter()
        .any(|value| value == "inconsistent_row_length"));

    run(&project, &["profile-source", recipe_arg()]);
    let profile: Value = serde_json::from_str(
        &fs::read_to_string(project.join("reports/source-profile.json")).unwrap(),
    )
    .unwrap();
    let farmer_id = profile["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["name"] == "farmer_id")
        .unwrap();
    assert_eq!(farmer_id["duplicate_value_count"], 1);
    assert_eq!(farmer_id["candidate_identifier"], false);
    assert_eq!(farmer_id["top_values"][0]["value"], "F-102");
    assert_eq!(farmer_id["top_values"][0]["count"], 2);
    let notes = profile["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["name"] == "notes")
        .unwrap();
    let notes_2 = profile["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["name"] == "notes_2")
        .unwrap();
    assert_eq!(notes["top_values"][0]["value"], "[redacted]");
    assert_eq!(notes_2["top_values"][0]["value"], "[redacted]");
    let patch: Value = serde_json::from_str(
        &fs::read_to_string(project.join("patches/source-profile.patch.json")).unwrap(),
    )
    .unwrap();
    assert!(patch
        .as_array()
        .unwrap()
        .iter()
        .all(|operation| operation["path"].as_str().unwrap() != "/fields/"));

    run(&project, &["preview-transform", recipe_arg()]);
    run(
        &project,
        &[
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ],
    );
    run(
        &project,
        &[
            "export-package",
            recipe_arg(),
            "--out",
            "target/messy-package",
        ],
    );

    let package = project.join("target/messy-package");
    assert!(!package.join("data/farmers-messy.csv").exists());
    let preview =
        fs::read_to_string(package.join("previews/canonical-samples.redacted.jsonl")).unwrap();
    assert!(preview.contains("[redacted]"));
    assert!(!preview.contains("Ada Sample"));
    assert_no_fixture_values(
        &package,
        &["Ada Sample", "Boris Sample", "Celine Sample", "Dana Sample"],
    );
}

#[test]
fn schema_output_text_inspection_and_row_selector_are_verified() {
    let (_temp, project) = fixture_project();

    let schema = output(&project, &["check-recipe", recipe_arg(), "--emit-schema"]);
    assert!(schema.contains("\"title\""));
    assert!(schema.contains("\"Recipe\""));

    let text = output(
        &project,
        &["inspect-source", "--format", "text", recipe_arg()],
    );
    assert!(text.contains("rows: 3"));
    assert!(text.contains("columns: 5"));
    let inspection: Value = serde_json::from_str(
        &fs::read_to_string(project.join("reports/source-inspection.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(inspection["source"]["encoding"], "utf-8");

    run(
        &project,
        &[
            "preview-transform",
            "--rows",
            "first:1",
            "--out",
            "target/first.jsonl",
            recipe_arg(),
        ],
    );
    let preview = fs::read_to_string(project.join("target/first.jsonl")).unwrap();
    assert_eq!(preview.lines().count(), 1);

    bin()
        .current_dir(&project)
        .args(["preview-transform", "--rows", "99", recipe_arg()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("row index 99 out of bounds"));

    fs::write(
        project.join("review.patch.json"),
        r#"[{"op":"replace","path":"/review/status","value":"reviewed"}]"#,
    )
    .unwrap();
    let patch_report = output(
        &project,
        &[
            "apply-patch",
            recipe_arg(),
            "--patch",
            "review.patch.json",
            "--out",
            "target/patched.recipe.yaml",
        ],
    );
    let patch_report: Value = serde_json::from_str(&patch_report).unwrap();
    assert_eq!(patch_report["operations"], 1);
    assert_eq!(patch_report["changed_paths"][0], "/review/status");
}

#[test]
fn init_refuses_existing_output_without_force_and_can_inspect_xlsx() {
    let (_temp, project) = fixture_project();

    bin()
        .current_dir(&project)
        .args([
            "init",
            "--source",
            "data/farmers.csv",
            "--out",
            "forge.recipe.yaml",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    bin()
        .current_dir(&project)
        .args([
            "init",
            "--source",
            "data/farmers.csv",
            "--project-name",
            "forced-project",
            "--out",
            "forge.recipe.yaml",
            "--force",
        ])
        .assert()
        .success();
    let forced: Value =
        serde_yaml::from_str(&fs::read_to_string(project.join(recipe_arg())).unwrap()).unwrap();
    assert_eq!(forced["project"]["name"], "forced-project");

    bin()
        .current_dir(&project)
        .args([
            "init",
            "--source",
            "data/farmers.xlsx",
            "--out",
            "xlsx.recipe.yaml",
        ])
        .assert()
        .success();
    let xlsx: Value =
        serde_yaml::from_str(&fs::read_to_string(project.join("xlsx.recipe.yaml")).unwrap())
            .unwrap();
    assert_eq!(xlsx["source"]["workbook"]["sheet"], "Farmers");
    run(&project, &["inspect-source", "xlsx.recipe.yaml"]);
}

#[test]
fn init_fails_when_profile_bundle_is_missing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project = temp.path();
    fs::create_dir_all(project.join("data")).unwrap();
    fs::write(project.join("data/source.csv"), "ID\n1\n").unwrap();

    bin()
        .current_dir(project)
        .args([
            "init",
            "--source",
            "data/source.csv",
            "--out",
            "forge.recipe.yaml",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("profile bundle file not found"));
}

#[test]
fn init_rejects_legacy_xls_sources() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project = temp.path();
    fs::create_dir_all(project.join("data")).unwrap();
    write_default_profile_bundle(project);
    fs::write(project.join("data/legacy.xls"), "not a real workbook").unwrap();

    bin()
        .current_dir(project)
        .args([
            "init",
            "--source",
            "data/legacy.xls",
            "--out",
            "forge.recipe.yaml",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "legacy .xls workbooks are not supported",
        ));
}

#[test]
fn scaffold_mapping_refuses_existing_output_without_force() {
    let (_temp, project) = fixture_project();

    run(
        &project,
        &[
            "scaffold-mapping",
            recipe_arg(),
            "--out",
            "mappings/scaffold.mapping.yaml",
        ],
    );
    bin()
        .current_dir(&project)
        .args([
            "scaffold-mapping",
            recipe_arg(),
            "--out",
            "mappings/scaffold.mapping.yaml",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
    run(
        &project,
        &[
            "scaffold-mapping",
            recipe_arg(),
            "--out",
            "mappings/scaffold.mapping.yaml",
            "--force",
        ],
    );
}

#[test]
fn csv_warnings_are_reported_without_failing_parse() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project = temp.path();
    fs::create_dir_all(project.join("data")).unwrap();
    write_default_profile_bundle(project);
    fs::write(
        project.join("data/warnings.csv"),
        b"\xEF\xBB\xBFName,Name,\nAlice,One,Extra\nBob,Two\n",
    )
    .unwrap();

    run(
        project,
        &[
            "init",
            "--source",
            "data/warnings.csv",
            "--out",
            "forge.recipe.yaml",
        ],
    );
    run(project, &["inspect-source", "forge.recipe.yaml"]);

    let report: Value = serde_json::from_str(
        &fs::read_to_string(project.join("reports/source-inspection.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["row_count"], 2);
    let warnings = report["warnings"].as_array().unwrap();
    assert!(warnings.iter().any(|value| value == "csv_bom_detected"));
    assert!(warnings.iter().any(|value| value == "blank_header"));
    assert!(warnings.iter().any(|value| value
        .as_str()
        .unwrap_or_default()
        .starts_with("duplicate_header")));
    assert!(warnings
        .iter()
        .any(|value| value == "inconsistent_row_length"));
}

#[test]
fn xlsx_requires_sheet_decision_and_reports_workbook_diagnostics() {
    let temp = tempfile::tempdir().expect("tempdir");
    let project = temp.path();
    fs::create_dir_all(project.join("data")).unwrap();
    write_default_profile_bundle(project);
    write_test_xlsx(&project.join("data/diagnostic.xlsx"), &["Main", "Archive"]);

    run(
        project,
        &[
            "init",
            "--source",
            "data/diagnostic.xlsx",
            "--out",
            "multi.recipe.yaml",
        ],
    );
    let multi: Value =
        serde_yaml::from_str(&fs::read_to_string(project.join("multi.recipe.yaml")).unwrap())
            .unwrap();
    assert_eq!(multi["source"]["workbook"]["sheet"], Value::Null);
    assert!(multi["source"]["workbook"]["decision_required"]
        .as_str()
        .unwrap()
        .contains("select one worksheet"));
    bin()
        .current_dir(project)
        .args(["inspect-source", "multi.recipe.yaml"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("workbook sheet is not selected"));

    run(
        project,
        &[
            "init",
            "--source",
            "data/diagnostic.xlsx",
            "--worksheet",
            "Main",
            "--out",
            "xlsx.recipe.yaml",
        ],
    );
    run(project, &["inspect-source", "xlsx.recipe.yaml"]);
    let report: Value = serde_json::from_str(
        &fs::read_to_string(project.join("reports/source-inspection.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["source"]["workbook"]["selected_sheet"], "Main");
    assert_eq!(report["source"]["workbook"]["sheets"][0], "Main");
    assert_eq!(report["source"]["workbook"]["sheets"][1], "Archive");
    let warnings = report["warnings"].as_array().unwrap();
    assert!(warnings
        .iter()
        .any(|value| value == "formula_cells_without_cached_values"));
    assert!(warnings
        .iter()
        .any(|value| value == "hidden_rows_or_columns_present"));
    assert!(warnings.iter().any(|value| value == "merged_cells_present"));
}

#[test]
fn sampled_profile_blocks_ready_candidate() {
    let (_temp, project) = fixture_project();
    run(&project, &["inspect-source", recipe_arg()]);
    run(
        &project,
        &["profile-source", "--row-limit", "1", recipe_arg()],
    );
    run(&project, &["preview-transform", recipe_arg()]);

    bin()
        .current_dir(&project)
        .args([
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("--row-limit"));
}

#[test]
fn require_status_not_ready_can_succeed() {
    let (_temp, project) = fixture_project();

    bin()
        .current_dir(&project)
        .args([
            "validate-output",
            "--require-status",
            "not_ready",
            recipe_arg(),
        ])
        .assert()
        .success();

    let report: Value = serde_json::from_str(
        &fs::read_to_string(project.join("reports/readiness-report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["status"], "not_ready");
}

#[test]
fn suggest_alignments_is_deterministic() {
    let (_temp, project) = fixture_project();
    run(&project, &["suggest-alignments", recipe_arg()]);
    let first_report = fs::read(project.join("reports/alignment-suggestions.json")).unwrap();
    let first_patch = fs::read(project.join("patches/alignment-suggestions.patch.json")).unwrap();
    run(&project, &["suggest-alignments", recipe_arg()]);
    assert_eq!(
        first_report,
        fs::read(project.join("reports/alignment-suggestions.json")).unwrap()
    );
    assert_eq!(
        first_patch,
        fs::read(project.join("patches/alignment-suggestions.patch.json")).unwrap()
    );
}

#[test]
fn preview_rejects_source_override_with_wrong_hash() {
    let (_temp, project) = fixture_project();
    fs::write(
        project.join("data/other.csv"),
        "Farmer ID,Full Name,District,Registration Status,Registration Date\nX-1,Wrong Demo,East,ACTIVE,2025-01-01\n",
    )
    .unwrap();

    bin()
        .current_dir(&project)
        .args([
            "preview-transform",
            "--source-override",
            "data/other.csv",
            recipe_arg(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("source hash mismatch"));
}

#[test]
fn mapping_changes_after_preview_block_ready_candidate() {
    let (_temp, project) = fixture_project();
    run(&project, &["profile-source", recipe_arg()]);
    run(&project, &["preview-transform", recipe_arg()]);
    let mapping = project.join("mappings/crosswalk.mapping.yaml");
    let text = fs::read_to_string(&mapping).unwrap().replace(
        r#"registration_status: "source.registration_status""#,
        r#"registration_status: "'CHANGED'""#,
    );
    fs::write(mapping, text).unwrap();

    bin()
        .current_dir(&project)
        .args([
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("preview mapping hash is stale"));
}

#[test]
fn required_target_metadata_without_canonical_field_blocks_ready_candidate() {
    let (_temp, project) = fixture_project();
    run(&project, &["profile-source", recipe_arg()]);
    let mapping = project.join("mappings/crosswalk.mapping.yaml");
    let text = fs::read_to_string(&mapping)
        .unwrap()
        .replace("farmer_identifier:", "farmer_identifier_removed:");
    fs::write(mapping, text).unwrap();
    run(&project, &["preview-transform", recipe_arg()]);

    bin()
        .current_dir(&project)
        .args([
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("missing canonical field"));
}

#[test]
fn draft_code_list_crosswalk_blocks_ready_candidate() {
    let (_temp, project) = fixture_project();
    replace_in_file(
        &project.join(recipe_arg()),
        "status: accepted\nmappings:",
        "status: draft\nmappings:",
    );
    run(&project, &["profile-source", recipe_arg()]);
    run(&project, &["preview-transform", recipe_arg()]);

    let output = bin()
        .current_dir(&project)
        .args([
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("code list crosswalk"))
        .get_output()
        .stdout
        .clone();
    let report: Value = serde_json::from_slice(&output).unwrap();
    let errors = report["errors"].as_array().unwrap();
    assert_eq!(
        errors
            .iter()
            .filter(|error| error
                .as_str()
                .unwrap_or_default()
                .contains("code list crosswalk district_codes is not accepted"))
            .count(),
        1
    );
}

#[test]
fn profile_redacts_recipe_sensitive_fields() {
    let (_temp, project) = fixture_project();
    replace_in_file(
        &project.join(recipe_arg()),
        "source_name: District\n    role: attribute\n    sensitivity: low",
        "source_name: District\n    role: attribute\n    sensitivity: high",
    );
    run(&project, &["profile-source", recipe_arg()]);
    let report = fs::read_to_string(project.join("reports/source-profile.json")).unwrap();
    assert!(report.contains("[redacted]"));
    assert!(!report.contains("North"));
    assert!(!report.contains("South"));
    let report: Value = serde_json::from_str(&report).unwrap();
    assert!(report["fields"]
        .as_array()
        .unwrap()
        .iter()
        .all(|field| field["duplicate_value_count"].is_number()));
}

#[test]
fn missing_profile_report_blocks_ready_candidate() {
    let (_temp, project) = fixture_project();
    run(&project, &["preview-transform", recipe_arg()]);

    bin()
        .current_dir(&project)
        .args([
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("source profile report is missing"));
}

#[test]
fn transform_diagnostics_errors_block_ready_candidate() {
    let (_temp, project) = fixture_project();
    run(&project, &["profile-source", recipe_arg()]);
    replace_in_file(
        &project.join("mappings/crosswalk.mapping.yaml"),
        r#"farmer_identifier: "source.farmer_id""#,
        r#"farmer_identifier: "source.missing_field""#,
    );
    run(&project, &["preview-transform", recipe_arg()]);
    let diagnostics: Value = serde_json::from_str(
        &fs::read_to_string(project.join("reports/transform-diagnostics.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        diagnostics["diagnostics"][0]["suggested_fix_class"],
        "check_mapping_expression"
    );

    bin()
        .current_dir(&project)
        .args([
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("transform diagnostic error"));
}

#[test]
fn preview_transform_fails_when_mapping_does_not_compile() {
    let (_temp, project) = fixture_project();
    fs::write(
        project.join("mappings/crosswalk.mapping.yaml"),
        "version: \"0.1\"\nrecords:\n  canonical:\n    fields:\n      farmer_identifier: \"source.\"\n",
    )
    .unwrap();

    bin()
        .current_dir(&project)
        .args(["preview-transform", recipe_arg()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Crosswalk compile failed"));
}

#[test]
fn field_code_list_without_matching_crosswalk_blocks_ready_candidate() {
    let (_temp, project) = fixture_project();
    replace_in_file(
        &project.join(recipe_arg()),
        "code_list: district_codes",
        "code_list: missing_codes",
    );
    run(&project, &["profile-source", recipe_arg()]);
    run(&project, &["preview-transform", recipe_arg()]);

    bin()
        .current_dir(&project)
        .args([
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("without a value crosswalk"));
}

#[test]
fn stale_profile_report_blocks_ready_candidate() {
    let (_temp, project) = fixture_project();
    run(&project, &["profile-source", recipe_arg()]);
    replace_in_file(
        &project.join(recipe_arg()),
        "source_name: District\n    role: attribute\n    sensitivity: low",
        "source_name: District\n    role: attribute\n    sensitivity: high",
    );
    run(&project, &["preview-transform", recipe_arg()]);

    bin()
        .current_dir(&project)
        .args([
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains(
            "source profile recipe hash is stale",
        ));
}

#[test]
fn check_recipe_rejects_unknown_sections_and_empty_source_hash() {
    let (_temp, project) = fixture_project();
    replace_in_file(
        &project.join(recipe_arg()),
        "version:",
        "unknown_section: true\nversion:",
    );
    bin()
        .current_dir(&project)
        .args(["check-recipe", recipe_arg()])
        .assert()
        .failure();

    let (_temp, project) = fixture_project();
    replace_in_file(
        &project.join(recipe_arg()),
        "value: 72e497effe7dc0c6de37789b607de153e8e27a39d54b23b61ed7c4e3f46f9fce",
        "value: \"\"",
    );
    bin()
        .current_dir(&project)
        .args(["check-recipe", recipe_arg()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("/source/hash/value"));
}

#[test]
fn check_recipe_rejects_profile_bundle_hash_mismatch() {
    let (_temp, project) = fixture_project();
    fs::write(
        project.join("profiles/publicschema-demo.bundle.json"),
        r#"{"id":"publicschema-demo","terms":[]}"#,
    )
    .unwrap();

    bin()
        .current_dir(&project)
        .args(["check-recipe", recipe_arg()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("profile bundle hash mismatch"));
}

#[test]
fn check_recipe_rejects_path_traversal() {
    let (_temp, project) = fixture_project();
    replace_in_file(
        &project.join(recipe_arg()),
        "path: data/farmers.csv",
        "path: ../outside.csv",
    );

    bin()
        .current_dir(&project)
        .args(["check-recipe", recipe_arg()])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "/source/path must be a relative path",
        ));
}

#[test]
fn source_override_rejects_path_traversal() {
    let (_temp, project) = fixture_project();

    bin()
        .current_dir(&project)
        .args([
            "inspect-source",
            "--source-override",
            "../outside.csv",
            recipe_arg(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "--source-override must be a relative path",
        ));
}

#[test]
fn validate_output_rejects_invalid_mapping_quality_and_on_missing() {
    let (_temp, project) = fixture_project();
    run(&project, &["profile-source", recipe_arg()]);
    replace_in_file(
        &project.join("mappings/crosswalk.mapping.yaml"),
        "quality: exact\n      reviewer: demo-reviewer",
        "quality: typo\n      on_missing: typo\n      reviewer: demo-reviewer",
    );
    run(&project, &["preview-transform", recipe_arg()]);

    bin()
        .current_dir(&project)
        .args([
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("mapping quality typo is invalid"))
        .stdout(predicate::str::contains("on_missing typo is invalid"));
}

#[test]
fn scaffolded_mapping_blocks_ready_candidate_until_reviewed() {
    let (_temp, project) = fixture_project();
    run(&project, &["profile-source", recipe_arg()]);
    run(
        &project,
        &[
            "scaffold-mapping",
            recipe_arg(),
            "--out",
            "mappings/scaffold.mapping.yaml",
        ],
    );
    patch_recipe_mapping_file(&project, "mappings/scaffold.mapping.yaml");
    run(&project, &["preview-transform", recipe_arg()]);

    bin()
        .current_dir(&project)
        .args([
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not_ready"));
}

#[test]
fn source_hash_mismatch_is_not_ready() {
    let (_temp, project) = fixture_project();
    fs::write(
        project.join("data/farmers.csv"),
        "Farmer ID,Full Name,District\nF-999,Changed Demo,East\n",
    )
    .unwrap();

    bin()
        .current_dir(&project)
        .args(["profile-source", recipe_arg()])
        .assert()
        .failure()
        .stderr(predicate::str::contains("source hash mismatch"));
}

#[test]
fn commands_do_not_modify_source_file() {
    let (_temp, project) = fixture_project();
    let source = project.join("data/farmers.csv");
    let before_hash = sha256_hex(&fs::read(&source).unwrap());
    let before_len = fs::metadata(&source).unwrap().len();
    let mut permissions = fs::metadata(&source).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&source, permissions).unwrap();

    run(&project, &["inspect-source", recipe_arg()]);
    run(&project, &["profile-source", recipe_arg()]);
    run(&project, &["suggest-alignments", recipe_arg()]);
    run(&project, &["preview-transform", recipe_arg()]);
    run(
        &project,
        &[
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ],
    );

    assert_eq!(before_hash, sha256_hex(&fs::read(&source).unwrap()));
    assert_eq!(before_len, fs::metadata(&source).unwrap().len());
}

#[test]
fn invalid_patch_leaves_recipe_bytes_unchanged() {
    let (_temp, project) = fixture_project();
    let recipe = project.join(recipe_arg());
    let before = fs::read(&recipe).unwrap();
    fs::write(
        project.join("bad.patch.json"),
        r#"[{"op":"remove","path":"/version"}]"#,
    )
    .unwrap();

    bin()
        .current_dir(&project)
        .args([
            "apply-patch",
            recipe_arg(),
            "--patch",
            "bad.patch.json",
            "--out",
            recipe_arg(),
        ])
        .assert()
        .failure();

    assert_eq!(before, fs::read(&recipe).unwrap());
}

#[test]
fn stale_preview_fails_required_ready_candidate() {
    let (_temp, project) = fixture_project();
    run(&project, &["profile-source", recipe_arg()]);
    run(&project, &["preview-transform", recipe_arg()]);
    fs::write(
        project.join("noop.patch.json"),
        r#"[{"op":"replace","path":"/review/status","value":"reviewed"}]"#,
    )
    .unwrap();
    run(
        &project,
        &[
            "apply-patch",
            recipe_arg(),
            "--patch",
            "noop.patch.json",
            "--out",
            recipe_arg(),
        ],
    );
    run(&project, &["profile-source", recipe_arg()]);

    bin()
        .current_dir(&project)
        .args([
            "validate-output",
            "--require-status",
            "ready_candidate",
            recipe_arg(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("ready_with_warnings"));
}

fn patch_recipe_mapping_file(project: &Path, mapping_file: &str) {
    let path = project.join(recipe_arg());
    let mut recipe: Value = serde_yaml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    recipe["mappings"]["file"] = Value::String(mapping_file.to_string());
    fs::write(&path, serde_yaml::to_string(&recipe).unwrap()).unwrap();
}

fn replace_in_file(path: &Path, from: &str, to: &str) {
    let text = fs::read_to_string(path).unwrap();
    assert!(
        text.contains(from),
        "expected {from:?} in {}",
        path.display()
    );
    fs::write(path, text.replace(from, to)).unwrap();
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn write_default_profile_bundle(project: &Path) {
    fs::create_dir_all(project.join("profiles")).unwrap();
    fs::copy(
        "fixtures/demo/profiles/publicschema-demo.bundle.json",
        project.join("profiles/publicschema-demo.bundle.json"),
    )
    .unwrap();
}

fn assert_no_fixture_values(package: &Path, forbidden: &[&str]) {
    for file in files_under(package) {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for value in forbidden {
            assert!(
                !text.contains(value),
                "unexpected fixture value {value:?} in {}",
                file.display()
            );
        }
    }
}

fn files_under(path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            files.extend(files_under(&path));
        } else {
            files.push(path);
        }
    }
    files
}

fn write_test_xlsx(path: &Path, sheet_names: &[&str]) {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    zip.start_file("[Content_Types].xml", options).unwrap();
    let mut overrides = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>"#,
    );
    for index in 1..=sheet_names.len() {
        overrides.push_str(&format!(
            r#"<Override PartName="/xl/worksheets/sheet{index}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#
        ));
    }
    overrides.push_str("</Types>");
    zip.write_all(overrides.as_bytes()).unwrap();

    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
    )
    .unwrap();

    zip.start_file("xl/workbook.xml", options).unwrap();
    let mut workbook = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>"#,
    );
    for (index, name) in sheet_names.iter().enumerate() {
        let sheet_id = index + 1;
        workbook.push_str(&format!(
            r#"<sheet name="{name}" sheetId="{sheet_id}" r:id="rId{sheet_id}"/>"#
        ));
    }
    workbook.push_str("</sheets></workbook>");
    zip.write_all(workbook.as_bytes()).unwrap();

    zip.start_file("xl/_rels/workbook.xml.rels", options)
        .unwrap();
    let mut relationships = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    for index in 1..=sheet_names.len() {
        relationships.push_str(&format!(
            r#"<Relationship Id="rId{index}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{index}.xml"/>"#
        ));
    }
    relationships.push_str("</Relationships>");
    zip.write_all(relationships.as_bytes()).unwrap();

    for index in 1..=sheet_names.len() {
        zip.start_file(format!("xl/worksheets/sheet{index}.xml"), options)
            .unwrap();
        let worksheet = if index == 1 {
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<cols><col min="1" max="1" width="12" hidden="1"/></cols>
<sheetData>
<row r="1"><c r="A1" t="inlineStr"><is><t>Name</t></is></c><c r="B1" t="inlineStr"><is><t>Score</t></is></c></row>
<row r="2" hidden="1"><c r="A2" t="inlineStr"><is><t>Alice</t></is></c><c r="B2"><f>1+1</f></c></row>
</sheetData>
<mergeCells count="1"><mergeCell ref="A1:B1"/></mergeCells>
</worksheet>"#
        } else {
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
<sheetData>
<row r="1"><c r="A1" t="inlineStr"><is><t>Name</t></is></c></row>
<row r="2"><c r="A2" t="inlineStr"><is><t>Archived</t></is></c></row>
</sheetData>
</worksheet>"#
        };
        zip.write_all(worksheet.as_bytes()).unwrap();
    }

    zip.finish().unwrap();
}
