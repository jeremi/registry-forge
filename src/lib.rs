use calamine::{Data, Reader};
use clap::{Parser, Subcommand, ValueEnum};
use crosswalk_core::{EvaluationInput, MappingRuntime, RuntimeOptions};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ForgeError {
    #[error("{0}")]
    Message(String),
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Yaml {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    #[error("{path}: {source}")]
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
}

pub type Result<T> = std::result::Result<T, ForgeError>;

#[derive(Parser, Debug)]
#[command(name = "registry-forge")]
#[command(about = "Local preparation CLI for registry source data")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Init {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        project_name: Option<String>,
        #[arg(long)]
        profile_bundle: Option<PathBuf>,
        #[arg(long)]
        worksheet: Option<String>,
        #[arg(long)]
        force: bool,
    },
    CheckRecipe {
        recipe: PathBuf,
        #[arg(long)]
        emit_schema: bool,
    },
    InspectSource {
        #[arg(long)]
        source_override: Option<PathBuf>,
        recipe: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
    },
    ProfileSource {
        #[arg(long)]
        row_limit: Option<usize>,
        #[arg(long)]
        source_override: Option<PathBuf>,
        recipe: PathBuf,
    },
    SuggestAlignments {
        recipe: PathBuf,
    },
    ApplyPatch {
        recipe: PathBuf,
        #[arg(long)]
        patch: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    ScaffoldMapping {
        recipe: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        force: bool,
    },
    PreviewTransform {
        #[arg(long)]
        rows: Option<String>,
        #[arg(long)]
        source_override: Option<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
        recipe: PathBuf,
    },
    ValidateOutput {
        #[arg(long)]
        require_status: Option<ReadinessStatus>,
        #[arg(long)]
        source_override: Option<PathBuf>,
        recipe: PathBuf,
    },
    ExportPackage {
        recipe: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Json,
    Text,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[value(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum ReadinessStatus {
    NotReady,
    ReadyWithWarnings,
    ReadyCandidate,
}

impl std::fmt::Display for ReadinessStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotReady => write!(f, "not_ready"),
            Self::ReadyWithWarnings => write!(f, "ready_with_warnings"),
            Self::ReadyCandidate => write!(f, "ready_candidate"),
        }
    }
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init {
            source,
            out,
            project_name,
            profile_bundle,
            worksheet,
            force,
        } => cmd_init(source, out, project_name, profile_bundle, worksheet, force),
        Command::CheckRecipe {
            recipe,
            emit_schema,
        } => cmd_check_recipe(&recipe, emit_schema),
        Command::InspectSource {
            source_override,
            recipe,
            format,
        } => cmd_inspect_source(&recipe, source_override.as_deref(), format),
        Command::ProfileSource {
            row_limit,
            source_override,
            recipe,
        } => cmd_profile_source(&recipe, source_override.as_deref(), row_limit),
        Command::SuggestAlignments { recipe } => cmd_suggest_alignments(&recipe),
        Command::ApplyPatch { recipe, patch, out } => cmd_apply_patch(&recipe, &patch, &out),
        Command::ScaffoldMapping { recipe, out, force } => {
            cmd_scaffold_mapping(&recipe, &out, force)
        }
        Command::PreviewTransform {
            rows,
            source_override,
            out,
            recipe,
        } => cmd_preview_transform(&recipe, source_override.as_deref(), rows.as_deref(), out),
        Command::ValidateOutput {
            require_status,
            source_override,
            recipe,
        } => cmd_validate_output(&recipe, source_override.as_deref(), require_status),
        Command::ExportPackage { recipe, out } => cmd_export_package(&recipe, &out),
    }
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub version: String,
    pub project: Project,
    pub source: Source,
    pub profile_bundle: ProfileBundle,
    #[serde(default)]
    pub fields: BTreeMap<String, FieldConfig>,
    #[serde(default)]
    pub semantic_alignments: Vec<SemanticAlignment>,
    #[serde(default)]
    pub value_crosswalks: BTreeMap<String, ValueCrosswalk>,
    pub mappings: MappingRef,
    pub validation: ValidationConfig,
    pub candidates: CandidateConfig,
    pub review: ReviewConfig,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub name: String,
    #[serde(default)]
    pub reviewers: Vec<Reviewer>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reviewer {
    pub id: String,
    pub name: String,
    pub role: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    #[serde(rename = "type")]
    pub source_type: String,
    pub path: PathBuf,
    pub format: String,
    pub hash: HashRef,
    #[serde(default)]
    pub workbook: Option<WorkbookConfig>,
    pub parser: ParserConfig,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WorkbookConfig {
    #[serde(default)]
    pub sheet: Option<String>,
    #[serde(default = "default_header_row")]
    pub header_row: usize,
    #[serde(default)]
    pub decision_required: Option<String>,
}

fn default_header_row() -> usize {
    1
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HashRef {
    pub algorithm: String,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ParserConfig {
    pub family: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileBundle {
    pub id: String,
    pub path: PathBuf,
    pub hash: HashRef,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FieldConfig {
    pub source_name: String,
    pub role: String,
    pub sensitivity: String,
    pub type_hint: String,
    #[serde(default)]
    pub code_list: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticAlignment {
    pub source_field: String,
    pub target: String,
    #[serde(default)]
    pub match_level: Option<String>,
    pub status: String,
    pub confidence: String,
    #[serde(default)]
    pub reviewer: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValueCrosswalk {
    pub source_field: String,
    pub target_code_list: String,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MappingRef {
    pub engine: String,
    pub file: PathBuf,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationConfig {
    #[serde(default)]
    pub required_targets: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateConfig {
    pub relay: StatusOnly,
    pub manifest: StatusOnly,
    pub notary: StatusOnly,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StatusOnly {
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewConfig {
    pub status: String,
}

#[derive(Debug)]
struct Table {
    headers: Vec<String>,
    stable_headers: Vec<String>,
    records: Vec<Vec<String>>,
    rows: Vec<BTreeMap<String, String>>,
    warnings: Vec<String>,
    total_rows: usize,
}

fn cmd_init(
    source: PathBuf,
    out: PathBuf,
    project_name: Option<String>,
    profile_bundle: Option<PathBuf>,
    worksheet: Option<String>,
    force: bool,
) -> Result<()> {
    if out.exists() && !force {
        return Err(msg(format!(
            "{} already exists; pass --force to overwrite",
            out.display()
        )));
    }
    let format = infer_format(&source)?;
    let source_hash = sha256_file(&source)?;
    let (family, workbook) = if format == "csv" {
        ("csv".to_string(), None)
    } else {
        let sheet_names = workbook_sheet_names(&source)?;
        let selected_sheet = worksheet.or_else(|| {
            if sheet_names.len() == 1 {
                sheet_names.first().cloned()
            } else {
                None
            }
        });
        let decision_required = if selected_sheet.is_none() && sheet_names.len() > 1 {
            Some(format!(
                "select one worksheet from: {}",
                sheet_names.join(", ")
            ))
        } else {
            None
        };
        (
            "calamine".to_string(),
            Some(WorkbookConfig {
                sheet: selected_sheet,
                header_row: 1,
                decision_required,
            }),
        )
    };
    let bundle =
        profile_bundle.unwrap_or_else(|| PathBuf::from("profiles/publicschema-demo.bundle.json"));
    if !bundle.exists() {
        return Err(msg(format!(
            "profile bundle file not found: {}",
            bundle.display()
        )));
    }
    let bundle_hash = sha256_file(&bundle)?;
    let recipe = Recipe {
        version: "forge.recipe.v1".into(),
        project: Project {
            name: project_name.unwrap_or_else(|| "registry-forge-project".into()),
            reviewers: vec![],
        },
        source: Source {
            id: "source.main".into(),
            source_type: "file".into(),
            path: source,
            format,
            hash: HashRef {
                algorithm: "sha256".into(),
                value: source_hash,
            },
            workbook,
            parser: ParserConfig {
                family,
                version: env!("CARGO_PKG_VERSION").into(),
            },
        },
        profile_bundle: ProfileBundle {
            id: "publicschema-demo".into(),
            path: bundle,
            hash: HashRef {
                algorithm: "sha256".into(),
                value: bundle_hash,
            },
        },
        fields: BTreeMap::new(),
        semantic_alignments: vec![],
        value_crosswalks: BTreeMap::new(),
        mappings: MappingRef {
            engine: "crosswalk".into(),
            file: PathBuf::from("mappings/crosswalk.mapping.yaml"),
        },
        validation: ValidationConfig {
            required_targets: vec![],
        },
        candidates: CandidateConfig {
            relay: StatusOnly {
                status: "draft".into(),
            },
            manifest: StatusOnly {
                status: "draft".into(),
            },
            notary: StatusOnly {
                status: "draft".into(),
            },
        },
        review: ReviewConfig {
            status: "draft".into(),
        },
    };
    validate_recipe(&recipe)?;
    write_yaml(&out, &recipe)
}

fn cmd_check_recipe(recipe_path: &Path, emit_schema: bool) -> Result<()> {
    if emit_schema {
        println!(
            "{}",
            serde_json::to_string_pretty(&recipe_schema()).map_err(|source| ForgeError::Json {
                path: PathBuf::from("<schema>"),
                source,
            })?
        );
        return Ok(());
    }
    validate_recipe_schema_file(recipe_path)?;
    let recipe = read_recipe(recipe_path)?;
    validate_recipe(&recipe)?;
    verify_profile_bundle_hash(recipe_path, &recipe)?;
    Ok(())
}

fn cmd_inspect_source(
    recipe_path: &Path,
    source_override: Option<&Path>,
    format: OutputFormat,
) -> Result<()> {
    let recipe = read_recipe(recipe_path)?;
    validate_recipe(&recipe)?;
    let source_file = source_path(recipe_path, &recipe, source_override)?;
    let table = read_table(recipe_path, &recipe, source_override, None)?;
    let workbook = workbook_inspection(&source_file, &recipe)?;
    let report = json!({
        "source": {
            "path": recipe.source.path,
            "format": recipe.source.format,
            "hash": sha256_file(&source_file)?,
            "encoding": "utf-8",
            "workbook": workbook,
        },
        "headers": table.headers,
        "stable_headers": table.stable_headers,
        "row_count": table.total_rows,
        "column_count": table.stable_headers.len(),
        "warnings": table.warnings,
    });
    let report_path = recipe_dir(recipe_path).join("reports/source-inspection.json");
    write_json(&report_path, &report)?;
    if matches!(format, OutputFormat::Text) {
        println!("rows: {}", report["row_count"]);
        println!("columns: {}", report["column_count"]);
    }
    Ok(())
}

fn cmd_profile_source(
    recipe_path: &Path,
    source_override: Option<&Path>,
    row_limit: Option<usize>,
) -> Result<()> {
    let recipe = read_recipe(recipe_path)?;
    validate_recipe(&recipe)?;
    let table = read_table(recipe_path, &recipe, source_override, row_limit)?;
    let source_hash = sha256_file(&source_path(recipe_path, &recipe, source_override)?)?;
    let recipe_hash = recipe_hash(&recipe, recipe_path)?;
    let mut fields = Vec::new();
    let sample_count = table.rows.len();
    for stable in &table.stable_headers {
        let values: Vec<String> = table
            .rows
            .iter()
            .filter_map(|row| row.get(stable).cloned())
            .collect();
        let missing = values.iter().filter(|v| v.trim().is_empty()).count();
        let distinct: BTreeSet<String> = values.iter().cloned().collect();
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for value in values.iter().filter(|v| !v.trim().is_empty()) {
            *counts.entry(value.clone()).or_default() += 1;
        }
        let duplicate_value_count: usize = counts
            .values()
            .filter(|count| **count > 1)
            .map(|count| count - 1)
            .sum();
        let mut counted_values = counts.into_iter().collect::<Vec<_>>();
        counted_values.sort_by(|(left_value, left_count), (right_value, right_count)| {
            right_count
                .cmp(left_count)
                .then_with(|| left_value.cmp(right_value))
        });
        let mut top_values: Vec<Value> = counted_values
            .into_iter()
            .take(10)
            .map(|(value, count)| json!({"value": value, "count": count}))
            .collect();
        let sensitivity = recipe
            .fields
            .get(stable)
            .map(|field| field.sensitivity.as_str())
            .unwrap_or("high");
        if sensitivity == "high" {
            top_values = vec![json!({"value": "[redacted]", "count": sample_count})];
        }
        fields.push(json!({
            "name": stable,
            "missing_count": missing,
            "distinct_count": distinct.len(),
            "duplicate_value_count": duplicate_value_count,
            "top_values": top_values,
            "inferred_type": infer_type(&values),
            "candidate_identifier": distinct.len() == sample_count && missing == 0,
            "candidate_code_list": distinct.len() <= 20,
            "sensitivity_hint": sensitivity,
        }));
    }
    let report = json!({
        "_forge": {
            "recipe_hash": recipe_hash,
            "source_hash": source_hash
        },
        "sampled": row_limit.is_some(),
        "sample_row_count": sample_count,
        "row_count": table.total_rows,
        "column_count": table.stable_headers.len(),
        "fields": fields,
        "warnings": table.warnings,
    });
    write_json(
        &recipe_dir(recipe_path).join("reports/source-profile.json"),
        &report,
    )?;
    let patch = profile_patch(&recipe, &table);
    write_json(
        &recipe_dir(recipe_path).join("patches/source-profile.patch.json"),
        &patch,
    )?;
    Ok(())
}

fn cmd_suggest_alignments(recipe_path: &Path) -> Result<()> {
    let recipe = read_recipe(recipe_path)?;
    validate_recipe(&recipe)?;
    verify_profile_bundle_hash(recipe_path, &recipe)?;
    let bundle_path = recipe_dir(recipe_path).join(&recipe.profile_bundle.path);
    let bundle: Value = serde_json::from_str(&read_to_string(&bundle_path)?).map_err(|source| {
        ForgeError::Json {
            path: bundle_path.clone(),
            source,
        }
    })?;
    let terms = bundle["terms"].as_array().cloned().unwrap_or_default();
    let existing: BTreeSet<(String, String)> = recipe
        .semantic_alignments
        .iter()
        .map(|a| (a.source_field.clone(), a.target.clone()))
        .collect();
    let mut suggestions = Vec::new();
    let mut patch = Vec::new();
    for (field_id, field) in &recipe.fields {
        if let Some(term) = best_term(&field.source_name, &terms) {
            let target = term["id"].as_str().unwrap_or_default().to_string();
            let suggestion = json!({
                "source_field": field_id,
                "target": target,
                "status": "suggested",
                "confidence": "high",
                "rationale": "normalized source label matched profile term alias",
                "evidence": {"source_name": field.source_name},
            });
            suggestions.push(suggestion);
            if !existing.contains(&(field_id.clone(), target.clone())) {
                patch.push(json!({
                    "op": "add",
                    "path": "/semantic_alignments/-",
                    "value": {
                        "source_field": field_id,
                        "target": target,
                        "match_level": "exact",
                        "status": "needs_review",
                        "confidence": "high"
                    }
                }));
            }
        }
    }
    write_json(
        &recipe_dir(recipe_path).join("reports/alignment-suggestions.json"),
        &json!({"suggestions": suggestions}),
    )?;
    write_json(
        &recipe_dir(recipe_path).join("patches/alignment-suggestions.patch.json"),
        &Value::Array(patch),
    )?;
    Ok(())
}

fn cmd_apply_patch(recipe_path: &Path, patch_path: &Path, out: &Path) -> Result<()> {
    let recipe = read_recipe(recipe_path)?;
    validate_recipe(&recipe)?;
    let patch_value: Value =
        serde_json::from_str(&read_to_string(patch_path)?).map_err(|source| ForgeError::Json {
            path: patch_path.to_path_buf(),
            source,
        })?;
    let changed_paths = patch_value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|operation| operation.get("path").and_then(|path| path.as_str()))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let patch: json_patch::Patch =
        serde_json::from_value(patch_value).map_err(|source| ForgeError::Json {
            path: patch_path.to_path_buf(),
            source,
        })?;
    let mut recipe_value = serde_json::to_value(&recipe).map_err(|source| ForgeError::Json {
        path: recipe_path.to_path_buf(),
        source,
    })?;
    json_patch::patch(&mut recipe_value, &patch)
        .map_err(|err| msg(format!("patch failed: {err}")))?;
    let updated: Recipe =
        serde_json::from_value(recipe_value).map_err(|source| ForgeError::Json {
            path: recipe_path.to_path_buf(),
            source,
        })?;
    validate_recipe(&updated)?;
    write_yaml_atomic(out, &updated)?;
    println!(
        "{}",
        json!({"operations": patch.0.len(), "changed_paths": changed_paths, "out": out})
    );
    Ok(())
}

fn cmd_scaffold_mapping(recipe_path: &Path, out: &Path, force: bool) -> Result<()> {
    if out.exists() && !force {
        return Err(msg(format!(
            "{} already exists; pass --force to overwrite",
            out.display()
        )));
    }
    let recipe = read_recipe(recipe_path)?;
    validate_recipe(&recipe)?;
    let accepted: Vec<_> = recipe
        .semantic_alignments
        .iter()
        .filter(|a| a.status == "accepted")
        .collect();
    if accepted.is_empty() {
        return Err(msg("no accepted semantic alignments exist"));
    }
    let mut forge_rules = serde_yaml::Mapping::new();
    let mut fields = serde_yaml::Mapping::new();
    for alignment in accepted {
        let source_field = &alignment.source_field;
        let target_field = target_field_name(&alignment.target);
        fields.insert(
            serde_yaml::Value::String(target_field),
            serde_yaml::Value::String(format!("source.{source_field}")),
        );
        let mut rule = serde_yaml::Mapping::new();
        rule.insert("quality".into(), "needs_review".into());
        rule.insert("generated_by".into(), "scaffold-mapping".into());
        forge_rules.insert(
            serde_yaml::Value::String(alignment.target.clone()),
            rule.into(),
        );
    }
    let doc = json!({
        "version": "0.1",
        "name": recipe.project.name,
        "errors": {"mode": "collect"},
        "x-forge": {"rules": forge_rules},
        "records": {"canonical": {"fields": fields}},
    });
    let text = serde_yaml::to_string(&doc).map_err(|source| ForgeError::Yaml {
        path: out.to_path_buf(),
        source,
    })?;
    MappingRuntime::new(RuntimeOptions::default())
        .compile_mapping(&text)
        .map_err(|err| {
            msg(format!(
                "generated Crosswalk scaffold failed to compile: {err:?}"
            ))
        })?;
    ensure_parent(out)?;
    fs::write(out, text).map_err(|source| ForgeError::Io {
        path: out.to_path_buf(),
        source,
    })
}

fn cmd_preview_transform(
    recipe_path: &Path,
    source_override: Option<&Path>,
    rows: Option<&str>,
    out: Option<PathBuf>,
) -> Result<()> {
    let recipe = read_recipe(recipe_path)?;
    validate_recipe(&recipe)?;
    let table = read_table(recipe_path, &recipe, source_override, None)?;
    let source_hash = sha256_file(&source_path(recipe_path, &recipe, source_override)?)?;
    let mapping_path = recipe_dir(recipe_path).join(&recipe.mappings.file);
    let mapping_text = read_to_string(&mapping_path)?;
    let mapping_hash = sha256_bytes(&mapping_text);
    let runtime = MappingRuntime::new(RuntimeOptions::default());
    let compiled = runtime
        .compile_mapping(&mapping_text)
        .map_err(|err| msg(format!("Crosswalk compile failed: {err:?}")))?;
    let selected = selected_rows(rows, table.rows.len())?;
    let recipe_hash = recipe_hash(&recipe, recipe_path)?;
    let sensitive = sensitive_output_fields(&recipe);
    let mut output_lines = Vec::new();
    let mut diagnostics = Vec::new();
    for index in selected {
        let source = json!(table.rows[index]);
        let out = runtime.evaluate(
            &compiled,
            EvaluationInput {
                source,
                context: json!({}),
            },
        );
        for err in out.errors {
            diagnostics.push(json!({
                "source_row": index,
                "rule_id": err.path.unwrap_or_else(|| "mapping".into()),
                "severity": "error",
                "message": "mapping evaluation failed",
                "suggested_fix_class": "check_mapping_expression",
            }));
        }
        if let Some(records) = out.records.get("canonical") {
            for record in records {
                let mut record = record.clone();
                redact_record(&mut record, &sensitive);
                output_lines.push(json!({
                    "_forge": {
                        "source_row": index,
                        "recipe_hash": recipe_hash,
                        "mapping_hash": mapping_hash,
                        "source_hash": source_hash
                    },
                    "record": record
                }));
            }
        }
    }
    let out_path = out.unwrap_or_else(|| {
        recipe_dir(recipe_path).join("previews/canonical-samples.redacted.jsonl")
    });
    write_jsonl(&out_path, &output_lines)?;
    write_json(
        &recipe_dir(recipe_path).join("reports/transform-diagnostics.json"),
        &json!({"diagnostics": diagnostics}),
    )?;
    Ok(())
}

fn cmd_validate_output(
    recipe_path: &Path,
    source_override: Option<&Path>,
    require_status: Option<ReadinessStatus>,
) -> Result<()> {
    let recipe = read_recipe(recipe_path)?;
    validate_recipe(&recipe)?;
    verify_profile_bundle_hash(recipe_path, &recipe)?;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let source = source_path(recipe_path, &recipe, source_override)?;
    let hash = sha256_file(&source)?;
    if hash != recipe.source.hash.value {
        errors.push(format!(
            "source hash mismatch: expected {}, got {hash}",
            recipe.source.hash.value
        ));
    }
    match latest_profile_metadata(recipe_path)? {
        None => errors.push("source profile report is missing".into()),
        Some(profile) => {
            if profile.sampled {
                errors.push("latest source profile was generated with --row-limit".into());
            }
            let current_recipe_hash = recipe_hash(&recipe, recipe_path)?;
            if profile.recipe_hash.as_deref() != Some(current_recipe_hash.as_str()) {
                errors.push("source profile recipe hash is stale".into());
            }
            if profile.source_hash.as_deref() != Some(hash.as_str()) {
                errors.push("source profile source hash is stale".into());
            }
        }
    }
    let mapping_path = recipe_dir(recipe_path).join(&recipe.mappings.file);
    let mapping_text = read_to_string(&mapping_path)?;
    let mapping_hash = sha256_bytes(&mapping_text);
    let runtime = MappingRuntime::new(RuntimeOptions::default());
    runtime
        .compile_mapping(&mapping_text)
        .map_err(|err| msg(format!("Crosswalk compile failed: {err:?}")))?;
    let mapping_meta = mapping_metadata(&mapping_text)?;
    for (field_id, field) in &recipe.fields {
        if let Some(code_list) = &field.code_list {
            match recipe.value_crosswalks.get(code_list) {
                None => errors.push(format!(
                    "field {field_id} declares code list {code_list} without a value crosswalk"
                )),
                Some(crosswalk) => {
                    if crosswalk.source_field != *field_id {
                        errors.push(format!(
                            "code list crosswalk {code_list} source_field does not match field {field_id}"
                        ));
                    }
                }
            }
        }
    }
    for target in &recipe.validation.required_targets {
        let expected_field = target_field_name(target);
        if !mapping_meta.canonical_fields.contains(&expected_field) {
            errors.push(format!(
                "required target {target} is missing canonical field {expected_field}"
            ));
        }
        match mapping_meta.rules.get(target) {
            None => errors.push(format!("required target {target} has no mapping metadata")),
            Some(meta) => {
                if let Some(quality) = &meta.quality {
                    if !matches!(
                        quality.as_str(),
                        "exact" | "close" | "lossy" | "uncertain" | "needs_review"
                    ) {
                        errors.push(format!(
                            "required target {target} mapping quality {quality} is invalid"
                        ));
                    }
                }
                if let Some(on_missing) = &meta.on_missing {
                    if !matches!(
                        on_missing.as_str(),
                        "error" | "skip" | "use_default" | "use_null"
                    ) {
                        errors.push(format!(
                            "required target {target} on_missing {on_missing} is invalid"
                        ));
                    }
                }
                if meta.quality.as_deref() == Some("needs_review")
                    || meta.generated_by.as_deref() == Some("scaffold-mapping")
                {
                    errors.push(format!("required target {target} mapping is not reviewed"));
                }
                if meta.reviewer.as_deref().unwrap_or_default().is_empty() {
                    errors.push(format!("required target {target} mapping has no reviewer"));
                }
            }
        }
    }
    for (name, crosswalk) in &recipe.value_crosswalks {
        if crosswalk.status != "accepted" {
            errors.push(format!("code list crosswalk {name} is not accepted"));
        }
    }
    for alignment in &recipe.semantic_alignments {
        if alignment.status == "accepted"
            && alignment.reviewer.as_deref().unwrap_or_default().is_empty()
        {
            errors.push(format!(
                "accepted alignment {} has no reviewer",
                alignment.target
            ));
        }
    }
    let preview_path = recipe_dir(recipe_path).join("previews/canonical-samples.redacted.jsonl");
    if preview_path.exists() {
        let recipe_hash = recipe_hash(&recipe, recipe_path)?;
        let preview_meta = preview_metadata(&preview_path)?;
        if preview_meta.recipe_hash.as_deref() != Some(recipe_hash.as_str()) {
            warnings.push("preview recipe hash is stale".to_string());
        }
        if preview_meta.mapping_hash.as_deref() != Some(mapping_hash.as_str()) {
            warnings.push("preview mapping hash is stale".to_string());
        }
        if preview_meta.source_hash.as_deref() != Some(hash.as_str()) {
            errors.push("preview source hash does not match current source".to_string());
        }
    } else {
        errors.push("preview output is missing".into());
    }
    for message in transform_diagnostic_errors(recipe_path)? {
        errors.push(message);
    }
    let status = if !errors.is_empty() {
        ReadinessStatus::NotReady
    } else if !warnings.is_empty() {
        ReadinessStatus::ReadyWithWarnings
    } else {
        ReadinessStatus::ReadyCandidate
    };
    let report = json!({
        "status": status,
        "errors": errors,
        "warnings": warnings,
    });
    write_json(
        &recipe_dir(recipe_path).join("reports/readiness-report.json"),
        &report,
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|source| ForgeError::Json {
            path: recipe_dir(recipe_path).join("reports/readiness-report.json"),
            source,
        })?
    );
    if let Some(required) = require_status {
        if status != required {
            return Err(msg(format!(
                "readiness status {status} did not match required {required}"
            )));
        }
        return Ok(());
    }
    if status == ReadinessStatus::NotReady {
        return Err(msg("readiness status not_ready"));
    }
    Ok(())
}

fn cmd_export_package(recipe_path: &Path, out: &Path) -> Result<()> {
    let recipe = read_recipe(recipe_path)?;
    validate_recipe(&recipe)?;
    verify_profile_bundle_hash(recipe_path, &recipe)?;
    fs::create_dir_all(out).map_err(|source| ForgeError::Io {
        path: out.to_path_buf(),
        source,
    })?;
    for dir in [
        "reports",
        "mappings",
        "previews",
        "profile-bundles",
        "candidates/relay",
        "candidates/manifest",
        "candidates/notary",
    ] {
        fs::create_dir_all(out.join(dir)).map_err(|source| ForgeError::Io {
            path: out.join(dir),
            source,
        })?;
    }
    copy_dir_files(
        &recipe_dir(recipe_path).join("reports"),
        &out.join("reports"),
    )?;
    copy_dir_files(
        &recipe_dir(recipe_path).join("previews"),
        &out.join("previews"),
    )?;
    let mapping_file = file_name_path(&recipe.mappings.file, "mapping file")?;
    let packaged_mapping = PathBuf::from("mappings").join(&mapping_file);
    copy_if_exists(
        &recipe_dir(recipe_path).join(&recipe.mappings.file),
        &out.join(&packaged_mapping),
    )?;
    let profile_file = file_name_path(&recipe.profile_bundle.path, "profile bundle")?;
    let packaged_profile = PathBuf::from("profile-bundles").join(&profile_file);
    copy_if_exists(
        &recipe_dir(recipe_path).join(&recipe.profile_bundle.path),
        &out.join(&packaged_profile),
    )?;
    let mut packaged_recipe = recipe.clone();
    packaged_recipe.mappings.file = packaged_mapping;
    packaged_recipe.profile_bundle.path = packaged_profile;
    write_yaml(&out.join("forge.recipe.yaml"), &packaged_recipe)?;
    let packaged_recipe_hash = recipe_hash(&packaged_recipe, &out.join("forge.recipe.yaml"))?;
    rewrite_packaged_recipe_hashes(out, &packaged_recipe_hash)?;
    write_json(
        &out.join("candidates/relay/candidate.json"),
        &json!({"status": "draft"}),
    )?;
    write_json(
        &out.join("candidates/manifest/candidate.json"),
        &json!({"status": "draft"}),
    )?;
    write_json(
        &out.join("candidates/notary/candidate.json"),
        &json!({"status": "draft"}),
    )?;
    let mut artifacts = BTreeMap::new();
    collect_artifact_hashes(out, out, &mut artifacts)?;
    let manifest = json!({
        "recipe_hash": packaged_recipe_hash,
        "source_hash": recipe.source.hash.value,
        "command_versions": {
            "registry-forge": env!("CARGO_PKG_VERSION")
        },
        "artifacts": artifacts,
    });
    write_json(&out.join("package-manifest.json"), &manifest)?;
    Ok(())
}

fn file_name_path(path: &Path, label: &str) -> Result<PathBuf> {
    path.file_name()
        .map(PathBuf::from)
        .ok_or_else(|| msg(format!("{label} path has no file name")))
}

fn rewrite_packaged_recipe_hashes(package: &Path, recipe_hash: &str) -> Result<()> {
    let profile_path = package.join("reports/source-profile.json");
    if profile_path.exists() {
        let mut profile: Value =
            serde_json::from_str(&read_to_string(&profile_path)?).map_err(|source| {
                ForgeError::Json {
                    path: profile_path.clone(),
                    source,
                }
            })?;
        profile["_forge"]["recipe_hash"] = Value::String(recipe_hash.to_string());
        write_json(&profile_path, &profile)?;
    }

    let preview_path = package.join("previews/canonical-samples.redacted.jsonl");
    if preview_path.exists() {
        let mut lines = Vec::new();
        for line in read_to_string(&preview_path)?.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let mut value: Value =
                serde_json::from_str(line).map_err(|source| ForgeError::Json {
                    path: preview_path.clone(),
                    source,
                })?;
            value["_forge"]["recipe_hash"] = Value::String(recipe_hash.to_string());
            lines.push(value);
        }
        write_jsonl(&preview_path, &lines)?;
    }
    Ok(())
}

fn read_recipe(path: &Path) -> Result<Recipe> {
    serde_yaml::from_str(&read_to_string(path)?).map_err(|source| ForgeError::Yaml {
        path: path.to_path_buf(),
        source,
    })
}

fn recipe_schema() -> Value {
    serde_json::to_value(schemars::schema_for!(Recipe)).expect("recipe schema serializes")
}

fn validate_recipe_schema_file(path: &Path) -> Result<()> {
    let raw_yaml: serde_yaml::Value =
        serde_yaml::from_str(&read_to_string(path)?).map_err(|source| ForgeError::Yaml {
            path: path.to_path_buf(),
            source,
        })?;
    let raw_json = serde_json::to_value(raw_yaml).map_err(|source| ForgeError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    let schema = recipe_schema();
    let compiled = jsonschema::JSONSchema::compile(&schema)
        .map_err(|err| msg(format!("internal recipe schema failed to compile: {err}")))?;
    let errors: Vec<String> = compiled
        .validate(&raw_json)
        .err()
        .into_iter()
        .flatten()
        .map(|err| format!("{}: {}", err.instance_path, err))
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(msg(errors.join("\n")))
    }
}

fn validate_recipe(recipe: &Recipe) -> Result<()> {
    let mut errors = Vec::new();
    if recipe.version != "forge.recipe.v1" {
        errors.push("/version must be forge.recipe.v1".to_string());
    }
    if recipe.source.source_type != "file" {
        errors.push("/source/type must be file".to_string());
    }
    for (path, label) in [
        (&recipe.source.path, "/source/path"),
        (&recipe.profile_bundle.path, "/profile_bundle/path"),
        (&recipe.mappings.file, "/mappings/file"),
    ] {
        if !is_safe_relative_path(path) {
            errors.push(format!(
                "{label} must be a relative path without parent segments"
            ));
        }
    }
    if recipe.source.hash.algorithm != "sha256" {
        errors.push("/source/hash/algorithm must be sha256".to_string());
    }
    if recipe.source.hash.value.is_empty() {
        errors.push("/source/hash/value is required".to_string());
    } else if recipe.source.hash.value.len() != 64
        || !recipe
            .source
            .hash
            .value
            .chars()
            .all(|ch| ch.is_ascii_hexdigit())
    {
        errors.push("/source/hash/value must be a sha256 hex digest".to_string());
    }
    if recipe.mappings.engine != "crosswalk" {
        errors.push("/mappings/engine must be crosswalk".to_string());
    }
    if !matches!(recipe.source.format.as_str(), "csv" | "xlsx") {
        errors.push("/source/format must be csv or xlsx".to_string());
    }
    let reviewers: BTreeSet<_> = recipe
        .project
        .reviewers
        .iter()
        .map(|r| r.id.as_str())
        .collect();
    for (index, alignment) in recipe.semantic_alignments.iter().enumerate() {
        if alignment.status == "accepted" {
            match alignment.reviewer.as_deref() {
                Some(id) if reviewers.contains(id) => {}
                Some(_) => errors.push(format!("/semantic_alignments/{index}/reviewer is unknown")),
                None => errors.push(format!("/semantic_alignments/{index}/reviewer is required")),
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(msg(errors.join("\n")))
    }
}

fn read_table(
    recipe_path: &Path,
    recipe: &Recipe,
    source_override: Option<&Path>,
    row_limit: Option<usize>,
) -> Result<Table> {
    let path = source_path(recipe_path, recipe, source_override)?;
    verify_source_hash(recipe, &path)?;
    let mut table = match recipe.source.format.as_str() {
        "csv" => read_csv_table(&path)?,
        "xlsx" => read_workbook_table(&path, recipe)?,
        other => return Err(msg(format!("unsupported source format {other}"))),
    };
    table.stable_headers = stable_headers_for(&table.headers, recipe);
    table.rows = table
        .records
        .iter()
        .map(|record| {
            let mut row = BTreeMap::new();
            for (idx, stable) in table.stable_headers.iter().enumerate() {
                row.insert(stable.clone(), record.get(idx).cloned().unwrap_or_default());
            }
            row
        })
        .collect();
    table.total_rows = table.records.len();
    if let Some(limit) = row_limit {
        table.records.truncate(limit);
        table.rows.truncate(limit);
    }
    Ok(table)
}

fn normalize_records(
    headers: &[String],
    raw_rows: impl IntoIterator<Item = Vec<String>>,
    warnings: &mut Vec<String>,
) -> Vec<Vec<String>> {
    raw_rows
        .into_iter()
        .map(|raw| {
            if raw.len() != headers.len() {
                warnings.push("inconsistent_row_length".into());
            }
            let mut record = Vec::with_capacity(headers.len());
            for idx in 0..headers.len() {
                record.push(raw.get(idx).cloned().unwrap_or_default());
            }
            record
        })
        .collect()
}

fn record_to_raw_row(headers: &[String], record: &[String]) -> BTreeMap<String, String> {
    let mut row = BTreeMap::new();
    let mut seen = BTreeMap::new();
    for (idx, header) in headers.iter().enumerate() {
        let count = seen.entry(header.clone()).or_insert(0usize);
        *count += 1;
        let key = if *count == 1 {
            header.clone()
        } else {
            format!("{header}#{count}")
        };
        row.insert(key, record.get(idx).cloned().unwrap_or_default());
    }
    row
}

fn raw_rows(headers: &[String], records: &[Vec<String>]) -> Vec<BTreeMap<String, String>> {
    records
        .iter()
        .map(|record| record_to_raw_row(headers, record))
        .collect()
}

fn read_csv_table(path: &Path) -> Result<Table> {
    let bytes = read_bytes(path)?;
    let mut warnings = Vec::new();
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        warnings.push("csv_bom_detected".into());
    }
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(bytes.as_slice());
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|err| msg(format!("CSV header parse failed: {err}")))?
        .iter()
        .map(ToOwned::to_owned)
        .collect();
    header_warnings(&headers, &mut warnings);
    let mut raw = Vec::new();
    for record in rdr.records() {
        let record = record.map_err(|err| msg(format!("CSV row parse failed: {err}")))?;
        raw.push(record.iter().map(ToOwned::to_owned).collect());
    }
    let records = normalize_records(&headers, raw, &mut warnings);
    let rows = raw_rows(&headers, &records);
    Ok(Table {
        stable_headers: vec![],
        total_rows: records.len(),
        headers,
        records,
        rows,
        warnings,
    })
}

fn read_workbook_table(path: &Path, recipe: &Recipe) -> Result<Table> {
    let mut workbook = calamine::open_workbook_auto(path)
        .map_err(|err| msg(format!("workbook open failed: {err}")))?;
    let workbook_cfg = recipe.source.workbook.clone().unwrap_or(WorkbookConfig {
        sheet: None,
        header_row: 1,
        decision_required: None,
    });
    let sheet = workbook_cfg
        .sheet
        .clone()
        .ok_or_else(|| msg("workbook sheet is not selected"))?;
    let range = workbook
        .worksheet_range(&sheet)
        .map_err(|err| msg(format!("worksheet read failed: {err}")))?;
    let header_idx = workbook_cfg.header_row.saturating_sub(1);
    let mut rows_iter = range.rows();
    let header_row = rows_iter
        .nth(header_idx)
        .ok_or_else(|| msg("header row not found"))?;
    let headers: Vec<String> = header_row.iter().map(cell_to_string).collect();
    let mut warnings = Vec::new();
    warnings.extend(workbook_xml_warnings(path)?);
    header_warnings(&headers, &mut warnings);
    let raw = range
        .rows()
        .skip(header_idx + 1)
        .map(|raw| raw.iter().map(cell_to_string).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let records = normalize_records(&headers, raw, &mut warnings);
    let rows = raw_rows(&headers, &records);
    Ok(Table {
        stable_headers: vec![],
        total_rows: records.len(),
        headers,
        records,
        rows,
        warnings,
    })
}

fn workbook_sheet_names(path: &Path) -> Result<Vec<String>> {
    let workbook = calamine::open_workbook_auto(path)
        .map_err(|err| msg(format!("workbook open failed: {err}")))?;
    Ok(workbook.sheet_names().to_vec())
}

fn workbook_inspection(path: &Path, recipe: &Recipe) -> Result<Value> {
    if recipe.source.format != "xlsx" {
        return Ok(Value::Null);
    }
    let workbook_cfg = recipe.source.workbook.clone().unwrap_or(WorkbookConfig {
        sheet: None,
        header_row: 1,
        decision_required: None,
    });
    Ok(json!({
        "sheets": workbook_sheet_names(path)?,
        "selected_sheet": workbook_cfg.sheet,
        "header_row": workbook_cfg.header_row,
        "decision_required": workbook_cfg.decision_required,
    }))
}

fn workbook_xml_warnings(path: &Path) -> Result<Vec<String>> {
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| !ext.eq_ignore_ascii_case("xlsx"))
        .unwrap_or(true)
    {
        return Ok(Vec::new());
    }
    let file = fs::File::open(path).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|err| msg(format!("workbook zip read failed: {err}")))?;
    let mut warnings = BTreeSet::new();
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|err| msg(format!("workbook zip entry read failed: {err}")))?;
        let name = file.name().to_string();
        if !name.starts_with("xl/worksheets/") || !name.ends_with(".xml") {
            continue;
        }
        let mut xml = String::new();
        file.read_to_string(&mut xml)
            .map_err(|err| msg(format!("worksheet XML read failed: {err}")))?;
        if xml.contains("<mergeCell") {
            warnings.insert("merged_cells_present".to_string());
        }
        if xml.contains(" hidden=\"1\"") {
            warnings.insert("hidden_rows_or_columns_present".to_string());
        }
        if worksheet_has_formula_without_cached_value(&xml) {
            warnings.insert("formula_cells_without_cached_values".to_string());
        }
    }
    Ok(warnings.into_iter().collect())
}

fn worksheet_has_formula_without_cached_value(xml: &str) -> bool {
    let mut rest = xml;
    while let Some(start) = rest.find("<c") {
        rest = &rest[start..];
        let Some(end) = rest.find("</c>") else {
            break;
        };
        let cell = &rest[..end + 4];
        let has_formula = cell.contains("<f>") || cell.contains("<f ") || cell.contains("<f/");
        if has_formula && !cell.contains("<v>") {
            return true;
        }
        rest = &rest[end + 4..];
    }
    false
}

fn header_warnings(headers: &[String], warnings: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for header in headers {
        if header.trim().is_empty() {
            warnings.push("blank_header".into());
        }
        if !seen.insert(header.trim().to_lowercase()) {
            warnings.push(format!("duplicate_header:{header}"));
        }
    }
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.clone(),
        Data::Float(value) => value.to_string(),
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) => value.clone(),
        Data::DurationIso(value) => value.clone(),
        Data::Error(value) => format!("{value:?}"),
    }
}

fn source_path(
    recipe_path: &Path,
    recipe: &Recipe,
    source_override: Option<&Path>,
) -> Result<PathBuf> {
    if let Some(path) = source_override {
        if !is_safe_relative_path(path) {
            return Err(msg(
                "--source-override must be a relative path without parent segments",
            ));
        }
    }
    let path = source_override
        .map(Path::to_path_buf)
        .unwrap_or_else(|| recipe_dir(recipe_path).join(&recipe.source.path));
    if path.exists() {
        Ok(path)
    } else {
        Err(msg(format!("source file not found: {}", path.display())))
    }
}

fn verify_source_hash(recipe: &Recipe, path: &Path) -> Result<()> {
    let hash = sha256_file(path)?;
    if hash == recipe.source.hash.value {
        Ok(())
    } else {
        Err(msg(format!(
            "source hash mismatch: expected {}, got {hash}",
            recipe.source.hash.value
        )))
    }
}

fn verify_profile_bundle_hash(recipe_path: &Path, recipe: &Recipe) -> Result<()> {
    if recipe.profile_bundle.hash.algorithm != "sha256" {
        return Err(msg("/profile_bundle/hash/algorithm must be sha256"));
    }
    if recipe.profile_bundle.hash.value.is_empty() {
        return Err(msg("/profile_bundle/hash/value is required"));
    }
    let path = recipe_dir(recipe_path).join(&recipe.profile_bundle.path);
    let hash = sha256_file(&path)?;
    if hash == recipe.profile_bundle.hash.value {
        Ok(())
    } else {
        Err(msg(format!(
            "profile bundle hash mismatch: expected {}, got {hash}",
            recipe.profile_bundle.hash.value
        )))
    }
}

fn recipe_dir(recipe_path: &Path) -> PathBuf {
    recipe_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn sha256_file(path: &Path) -> Result<String> {
    Ok(sha256_bytes(&read_bytes(path)?))
}

fn sha256_bytes(bytes: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(bytes.as_ref()))
}

fn recipe_hash(recipe: &Recipe, recipe_path: &Path) -> Result<String> {
    Ok(sha256_bytes(&serde_yaml::to_string(recipe).map_err(
        |source| ForgeError::Yaml {
            path: recipe_path.to_path_buf(),
            source,
        },
    )?))
}

fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_to_string(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    ensure_parent(path)?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|source| ForgeError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    fs::write(path, bytes).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_jsonl(path: &Path, values: &[Value]) -> Result<()> {
    ensure_parent(path)?;
    let mut body = String::new();
    for value in values {
        body.push_str(
            &serde_json::to_string(value).map_err(|source| ForgeError::Json {
                path: path.to_path_buf(),
                source,
            })?,
        );
        body.push('\n');
    }
    fs::write(path, body).map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_yaml<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    ensure_parent(path)?;
    fs::write(
        path,
        serde_yaml::to_string(value).map_err(|source| ForgeError::Yaml {
            path: path.to_path_buf(),
            source,
        })?,
    )
    .map_err(|source| ForgeError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_yaml_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    ensure_parent(path)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|source| ForgeError::Io {
        path: parent.to_path_buf(),
        source,
    })?;
    let body = serde_yaml::to_string(value).map_err(|source| ForgeError::Yaml {
        path: path.to_path_buf(),
        source,
    })?;
    tmp.write_all(body.as_bytes())
        .map_err(|source| ForgeError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    tmp.persist(path).map_err(|err| ForgeError::Io {
        path: path.to_path_buf(),
        source: err.error,
    })?;
    Ok(())
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ForgeError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

fn infer_format(path: &Path) -> Result<String> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "csv" => Ok("csv".into()),
        "xlsx" => Ok("xlsx".into()),
        "xls" => Err(msg(
            "legacy .xls workbooks are not supported in the MVP; convert to .xlsx",
        )),
        other => Err(msg(format!("unsupported source extension {other}"))),
    }
}

fn is_safe_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn source_to_stable_fields(recipe: &Recipe) -> BTreeMap<String, String> {
    recipe
        .fields
        .iter()
        .map(|(id, field)| (field.source_name.clone(), id.clone()))
        .collect()
}

fn stable_headers_for(headers: &[String], recipe: &Recipe) -> Vec<String> {
    let source_to_stable = source_to_stable_fields(recipe);
    let mut seen = BTreeMap::new();
    headers
        .iter()
        .enumerate()
        .map(|(idx, header)| {
            let base = source_to_stable
                .get(header)
                .cloned()
                .unwrap_or_else(|| sanitize(header));
            let base = if base.is_empty() {
                format!("column_{}", idx + 1)
            } else {
                base
            };
            let count = seen.entry(base.clone()).or_insert(0usize);
            *count += 1;
            if *count == 1 {
                base
            } else {
                format!("{base}_{count}")
            }
        })
        .collect()
}

fn sanitize(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

fn sensitivity_hint(field: &str) -> &'static str {
    let field = field.to_lowercase();
    if field.contains("name") || field.contains("dob") || field.contains("birth") {
        "high"
    } else {
        "low"
    }
}

fn infer_type(values: &[String]) -> &'static str {
    let non_empty: Vec<_> = values.iter().filter(|v| !v.trim().is_empty()).collect();
    if non_empty.iter().all(|v| v.parse::<f64>().is_ok()) {
        "number"
    } else if non_empty
        .iter()
        .all(|v| v.len() == 10 && v.chars().nth(4) == Some('-'))
    {
        "date"
    } else {
        "string"
    }
}

fn profile_patch(recipe: &Recipe, table: &Table) -> Value {
    let mut ops = Vec::new();
    for (idx, stable) in table.stable_headers.iter().enumerate() {
        if !recipe.fields.contains_key(stable) {
            ops.push(json!({
                "op": "add",
                "path": format!("/fields/{stable}"),
                "value": {
                    "source_name": table.headers[idx],
                    "role": if stable.contains("id") { "identifier" } else { "attribute" },
                    "sensitivity": sensitivity_hint(stable),
                    "type_hint": "string"
                }
            }));
        }
    }
    Value::Array(ops)
}

fn best_term<'a>(label: &str, terms: &'a [Value]) -> Option<&'a Value> {
    let label = normalize(label);
    terms.iter().find(|term| {
        normalize(term["label"].as_str().unwrap_or_default()) == label
            || term["aliases"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|alias| normalize(alias.as_str().unwrap_or_default()) == label)
    })
}

fn normalize(input: &str) -> String {
    input
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn target_field_name(target: &str) -> String {
    target
        .split(':')
        .next_back()
        .unwrap_or(target)
        .replace(['.', '-'], "_")
}

#[derive(Default)]
struct MappingMeta {
    quality: Option<String>,
    on_missing: Option<String>,
    reviewer: Option<String>,
    generated_by: Option<String>,
}

#[derive(Default)]
struct MappingMetadata {
    rules: BTreeMap<String, MappingMeta>,
    canonical_fields: BTreeSet<String>,
}

fn mapping_metadata(text: &str) -> Result<MappingMetadata> {
    let yaml: serde_yaml::Value =
        serde_yaml::from_str(text).map_err(|source| ForgeError::Yaml {
            path: PathBuf::from("<mapping>"),
            source,
        })?;
    let mut out = MappingMetadata::default();
    if let Some(fields) = yaml
        .get("records")
        .and_then(|v| v.get("canonical"))
        .and_then(|v| v.get("fields"))
        .and_then(|v| v.as_mapping())
    {
        for (field, _) in fields {
            if let Some(field) = field.as_str() {
                out.canonical_fields.insert(field.to_string());
            }
        }
    }
    let Some(rules) = yaml
        .get("x-forge")
        .and_then(|v| v.get("rules"))
        .and_then(|v| v.as_mapping())
    else {
        return Ok(out);
    };
    for (target, value) in rules {
        if let Some(target) = target.as_str() {
            let meta = MappingMeta {
                quality: value
                    .get("quality")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                on_missing: value
                    .get("on_missing")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                reviewer: value
                    .get("reviewer")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                generated_by: value
                    .get("generated_by")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            };
            out.rules.insert(target.to_string(), meta);
        }
    }
    Ok(out)
}

struct ProfileMetadata {
    sampled: bool,
    recipe_hash: Option<String>,
    source_hash: Option<String>,
}

fn latest_profile_metadata(recipe_path: &Path) -> Result<Option<ProfileMetadata>> {
    let path = recipe_dir(recipe_path).join("reports/source-profile.json");
    if !path.exists() {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(&read_to_string(&path)?)
        .map_err(|source| ForgeError::Json { path, source })?;
    Ok(Some(ProfileMetadata {
        sampled: value["sampled"].as_bool().unwrap_or(false),
        recipe_hash: value["_forge"]["recipe_hash"].as_str().map(str::to_string),
        source_hash: value["_forge"]["source_hash"].as_str().map(str::to_string),
    }))
}

#[derive(Default)]
struct PreviewMetadata {
    recipe_hash: Option<String>,
    mapping_hash: Option<String>,
    source_hash: Option<String>,
}

fn preview_metadata(path: &Path) -> Result<PreviewMetadata> {
    let content = read_to_string(path)?;
    let Some(first_line) = content.lines().find(|line| !line.trim().is_empty()) else {
        return Ok(PreviewMetadata::default());
    };
    let value: Value = serde_json::from_str(first_line).map_err(|source| ForgeError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(PreviewMetadata {
        recipe_hash: value["_forge"]["recipe_hash"].as_str().map(str::to_string),
        mapping_hash: value["_forge"]["mapping_hash"].as_str().map(str::to_string),
        source_hash: value["_forge"]["source_hash"].as_str().map(str::to_string),
    })
}

fn transform_diagnostic_errors(recipe_path: &Path) -> Result<Vec<String>> {
    let path = recipe_dir(recipe_path).join("reports/transform-diagnostics.json");
    if !path.exists() {
        return Ok(vec!["transform diagnostics report is missing".into()]);
    }
    let value: Value = serde_json::from_str(&read_to_string(&path)?)
        .map_err(|source| ForgeError::Json { path, source })?;
    Ok(value["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|diagnostic| diagnostic["severity"].as_str() == Some("error"))
        .map(|diagnostic| {
            format!(
                "transform diagnostic error: {}",
                diagnostic["message"]
                    .as_str()
                    .unwrap_or("unknown transform error")
            )
        })
        .collect())
}

fn selected_rows(selector: Option<&str>, row_count: usize) -> Result<Vec<usize>> {
    match selector {
        None => Ok((0..row_count.min(20)).collect()),
        Some("all") => Ok((0..row_count).collect()),
        Some(value) if value.starts_with("first:") => {
            let n = value[6..]
                .parse::<usize>()
                .map_err(|_| msg("invalid first:N row selector"))?;
            Ok((0..row_count.min(n)).collect())
        }
        Some(value) => value
            .split(',')
            .map(|part| {
                let idx = part
                    .parse::<usize>()
                    .map_err(|_| msg("invalid row index"))?;
                if idx >= row_count {
                    Err(msg(format!("row index {idx} out of bounds")))
                } else {
                    Ok(idx)
                }
            })
            .collect(),
    }
}

fn sensitive_output_fields(recipe: &Recipe) -> BTreeSet<String> {
    let sensitive_sources: BTreeSet<_> = recipe
        .fields
        .iter()
        .filter(|(_, field)| field.sensitivity == "high")
        .map(|(id, _)| id.as_str())
        .collect();
    recipe
        .semantic_alignments
        .iter()
        .filter(|alignment| sensitive_sources.contains(alignment.source_field.as_str()))
        .map(|alignment| target_field_name(&alignment.target))
        .collect()
}

fn redact_record(value: &mut Value, sensitive: &BTreeSet<String>) {
    if let Value::Object(map) = value {
        for key in sensitive {
            if map.contains_key(key) {
                map.insert(key.clone(), Value::String("[redacted]".into()));
            }
        }
    }
}

fn copy_if_exists(from: &Path, to: &Path) -> Result<()> {
    if from.exists() {
        ensure_parent(to)?;
        fs::copy(from, to).map_err(|source| ForgeError::Io {
            path: to.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

fn copy_dir_files(from: &Path, to: &Path) -> Result<()> {
    if !from.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(from).map_err(|source| ForgeError::Io {
        path: from.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ForgeError::Io {
            path: from.to_path_buf(),
            source,
        })?;
        if entry
            .file_type()
            .map_err(|source| ForgeError::Io {
                path: entry.path(),
                source,
            })?
            .is_file()
        {
            copy_if_exists(&entry.path(), &to.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn collect_artifact_hashes(
    root: &Path,
    dir: &Path,
    out: &mut BTreeMap<String, String>,
) -> Result<()> {
    for entry in fs::read_dir(dir).map_err(|source| ForgeError::Io {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| ForgeError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if entry
            .file_type()
            .map_err(|source| ForgeError::Io {
                path: path.clone(),
                source,
            })?
            .is_dir()
        {
            collect_artifact_hashes(root, &path, out)?;
        } else if path.file_name().and_then(|n| n.to_str()) != Some("package-manifest.json") {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            out.insert(rel, sha256_file(&path)?);
        }
    }
    Ok(())
}

fn msg(message: impl Into<String>) -> ForgeError {
    ForgeError::Message(message.into())
}
