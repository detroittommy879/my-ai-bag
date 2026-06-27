use crate::{
    pack::{decrypt_payload, encrypt_payload},
    scan::{Category, ScanReport, visit_limited},
};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

const MAX_CONFIG_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct CredentialVault {
    pub version: u32,
    pub created_by: String,
    pub scanned_files: usize,
    pub credentials: Vec<CredentialEntry>,
    pub models: Vec<ModelEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CredentialEntry {
    pub tool_key: String,
    pub source_path: PathBuf,
    pub field_path: String,
    pub provider: Option<String>,
    pub value_kind: SecretValueKind,
    pub value: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum SecretValueKind {
    Literal,
    EnvironmentReference,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModelEntry {
    pub tool_key: String,
    pub source_path: PathBuf,
    pub field_path: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialPreview {
    pub scanned_files: usize,
    pub skipped_files: usize,
    pub credentials: Vec<RedactedCredential>,
    pub models: Vec<ModelEntry>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactedCredential {
    pub tool_key: String,
    pub source_path: PathBuf,
    pub field_path: String,
    pub provider: Option<String>,
    pub value_kind: SecretValueKind,
}

pub struct InventoryResult {
    pub vault: CredentialVault,
    pub preview: CredentialPreview,
}

pub fn discover_credentials(report: &ScanReport, selected_keys: &[String]) -> InventoryResult {
    let selected = selected_keys
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut credentials = Vec::new();
    let mut models = Vec::new();
    let mut scanned_files = 0;
    let mut skipped_files = 0;

    for tool in report
        .tools
        .iter()
        .filter(|tool| selected.contains(tool.key.as_str()))
    {
        let mut candidates = BTreeSet::new();
        for root in &tool.roots {
            collect_candidate_files(&root.path, true, &mut candidates);
        }
        for found in &tool.found {
            if !matches!(
                found.category,
                Category::Auth | Category::Setting | Category::Mcp
            ) {
                continue;
            }
            collect_candidate_files(&found.path, found.is_dir, &mut candidates);
        }

        for path in candidates {
            match parse_candidate(&tool.key, &path, &mut credentials, &mut models) {
                Ok(true) => scanned_files += 1,
                Ok(false) | Err(_) => skipped_files += 1,
            }
        }
    }

    credentials.sort_by(|left, right| {
        left.tool_key
            .cmp(&right.tool_key)
            .then(left.source_path.cmp(&right.source_path))
            .then(left.field_path.cmp(&right.field_path))
    });
    credentials.dedup_by(|left, right| {
        left.tool_key == right.tool_key
            && left.source_path == right.source_path
            && left.field_path == right.field_path
            && left.value == right.value
    });
    models.sort();
    models.dedup();

    let redacted = credentials
        .iter()
        .map(|entry| RedactedCredential {
            tool_key: entry.tool_key.clone(),
            source_path: entry.source_path.clone(),
            field_path: entry.field_path.clone(),
            provider: entry.provider.clone(),
            value_kind: entry.value_kind,
        })
        .collect();

    InventoryResult {
        vault: CredentialVault {
            version: 1,
            created_by: "my-ai-bag-floem prototype".to_string(),
            scanned_files,
            credentials,
            models: models.clone(),
        },
        preview: CredentialPreview {
            scanned_files,
            skipped_files,
            credentials: redacted,
            models,
            notes: vec![
                "Credential values are redacted from terminal and JSON preview output.".to_string(),
                "Vault export is local and encrypted; no upload happens.".to_string(),
                "Detection is heuristic. Review source paths before relying on this inventory."
                    .to_string(),
                "Credential injection into agent configs is not implemented yet.".to_string(),
            ],
        },
    }
}

pub fn write_credential_vault(
    vault: &CredentialVault,
    output: &Path,
    passphrase: &str,
) -> Result<()> {
    if passphrase.len() < 12 {
        return Err(anyhow!("passphrase must be at least 12 characters"));
    }
    if vault.credentials.is_empty() && vault.models.is_empty() {
        return Err(anyhow!("no credentials or models were found to store"));
    }

    let payload = serde_json::to_vec_pretty(vault)?;
    let encrypted = encrypt_payload(&payload, passphrase)?;
    fs::write(output, encrypted).with_context(|| format!("failed to write {}", output.display()))
}

pub fn decrypt_credential_vault_bytes(
    encrypted: &[u8],
    passphrase: &str,
) -> Result<CredentialVault> {
    Ok(serde_json::from_slice(&decrypt_payload(
        encrypted, passphrase,
    )?)?)
}

pub fn format_credential_preview(preview: &CredentialPreview) -> String {
    let mut lines = vec![
        "My AI Bag credential/model preview".to_string(),
        format!("Config files parsed: {}", preview.scanned_files),
        format!("Files skipped or unreadable: {}", preview.skipped_files),
        String::new(),
        format!("Credential fields found: {}", preview.credentials.len()),
    ];

    for entry in &preview.credentials {
        let provider = entry.provider.as_deref().unwrap_or("unknown provider");
        lines.push(format!(
            "- {} | {} | {} | {:?} | {}",
            entry.tool_key,
            provider,
            entry.field_path,
            entry.value_kind,
            entry.source_path.display()
        ));
    }
    if preview.credentials.is_empty() {
        lines.push("- none".to_string());
    }

    lines.push(String::new());
    lines.push(format!("Model references found: {}", preview.models.len()));
    for entry in &preview.models {
        lines.push(format!(
            "- {} | {} | {} | {}",
            entry.tool_key,
            entry.model,
            entry.field_path,
            entry.source_path.display()
        ));
    }
    if preview.models.is_empty() {
        lines.push("- none".to_string());
    }

    lines.push(String::new());
    lines.push("Notes:".to_string());
    lines.extend(preview.notes.iter().map(|note| format!("- {note}")));
    lines.join("\n")
}

fn collect_candidate_files(path: &Path, is_dir: bool, candidates: &mut BTreeSet<PathBuf>) {
    if !is_dir {
        if is_supported_config(path) {
            candidates.insert(path.to_path_buf());
        }
        return;
    }

    visit_limited(path, 0, 8, &mut |candidate, candidate_is_dir| {
        if !candidate_is_dir && is_supported_config(candidate) {
            candidates.insert(candidate.to_path_buf());
        }
    });
}

fn is_supported_config(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name == ".env" || name.starts_with(".env.") {
        return true;
    }
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "json" | "toml" | "yaml" | "yml"
    )
}

fn parse_candidate(
    tool_key: &str,
    path: &Path,
    credentials: &mut Vec<CredentialEntry>,
    models: &mut Vec<ModelEntry>,
) -> Result<bool> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_CONFIG_BYTES {
        return Ok(false);
    }
    let contents = fs::read_to_string(path)?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if name == ".env" || name.starts_with(".env.") {
        parse_env(tool_key, path, &contents, credentials);
        return Ok(true);
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let value = match extension.as_str() {
        "json" => serde_json::from_str::<Value>(&contents)?,
        "toml" => serde_json::to_value(toml::from_str::<toml::Value>(&contents)?)?,
        "yaml" | "yml" => {
            serde_json::to_value(serde_yaml::from_str::<serde_yaml::Value>(&contents)?)?
        }
        _ => return Ok(false),
    };

    walk_value(tool_key, path, &mut Vec::new(), &value, credentials, models);
    Ok(true)
}

fn parse_env(tool_key: &str, path: &Path, contents: &str, credentials: &mut Vec<CredentialEntry>) {
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !is_secret_field(key) {
            continue;
        }
        let value = raw_value
            .trim()
            .trim_matches(|character| character == '\'' || character == '"')
            .to_string();
        if value.is_empty() {
            continue;
        }
        credentials.push(CredentialEntry {
            tool_key: tool_key.to_string(),
            source_path: path.to_path_buf(),
            field_path: key.to_string(),
            provider: provider_hint(key),
            value_kind: value_kind(&value),
            value,
        });
    }
}

fn walk_value(
    tool_key: &str,
    source_path: &Path,
    field_path: &mut Vec<String>,
    value: &Value,
    credentials: &mut Vec<CredentialEntry>,
    models: &mut Vec<ModelEntry>,
) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                field_path.push(key.clone());
                let joined = field_path.join(".");
                if is_secret_field(key)
                    && let Some(secret) = child.as_str()
                    && !secret.is_empty()
                {
                    credentials.push(CredentialEntry {
                        tool_key: tool_key.to_string(),
                        source_path: source_path.to_path_buf(),
                        field_path: joined.clone(),
                        provider: provider_hint(&joined),
                        value_kind: value_kind(secret),
                        value: secret.to_string(),
                    });
                }
                if is_model_field(key) {
                    collect_models(tool_key, source_path, &joined, child, models);
                }
                walk_value(
                    tool_key,
                    source_path,
                    field_path,
                    child,
                    credentials,
                    models,
                );
                field_path.pop();
            }
        }
        Value::Array(items) => {
            for item in items {
                walk_value(tool_key, source_path, field_path, item, credentials, models);
            }
        }
        _ => {}
    }
}

fn collect_models(
    tool_key: &str,
    source_path: &Path,
    field_path: &str,
    value: &Value,
    models: &mut Vec<ModelEntry>,
) {
    let mut add = |model: &str| {
        let model = model.trim();
        if !model.is_empty() && model.len() <= 256 {
            models.push(ModelEntry {
                tool_key: tool_key.to_string(),
                source_path: source_path.to_path_buf(),
                field_path: field_path.to_string(),
                model: model.to_string(),
            });
        }
    };

    match value {
        Value::String(model) => add(model),
        Value::Array(items) => {
            for item in items {
                if let Some(model) = item.as_str() {
                    add(model);
                } else if let Some(model) = item.get("id").and_then(Value::as_str) {
                    add(model);
                } else if let Some(model) = item.get("name").and_then(Value::as_str) {
                    add(model);
                }
            }
        }
        Value::Object(object) => {
            for model in object.keys() {
                add(model);
            }
        }
        _ => {}
    }
}

fn normalized_field(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_secret_field(field: &str) -> bool {
    let field = normalized_field(field);
    field.contains("apikey")
        || matches!(
            field.as_str(),
            "token"
                | "auth"
                | "authorization"
                | "authtoken"
                | "accesstoken"
                | "refreshtoken"
                | "bearertoken"
                | "secret"
                | "clientsecret"
                | "accesskey"
                | "secretkey"
                | "password"
                | "credential"
                | "credentials"
        )
        || field.ends_with("token")
        || field.ends_with("secret")
        || field.ends_with("password")
        || field.ends_with("accesskeyid")
        || field.ends_with("secretaccesskey")
        || field.ends_with("clientsecret")
}

fn is_model_field(field: &str) -> bool {
    matches!(
        normalized_field(field).as_str(),
        "model" | "models" | "modelid" | "modelname" | "defaultmodel" | "modellist"
    )
}

fn value_kind(value: &str) -> SecretValueKind {
    let trimmed = value.trim();
    if trimmed.starts_with('$')
        || trimmed.starts_with("env:")
        || (trimmed.starts_with("{{") && trimmed.ends_with("}}"))
    {
        SecretValueKind::EnvironmentReference
    } else {
        SecretValueKind::Literal
    }
}

fn provider_hint(value: &str) -> Option<String> {
    let value = normalized_field(value);
    [
        "openrouter",
        "anthropic",
        "openai",
        "gemini",
        "google",
        "mistral",
        "deepseek",
        "moonshot",
        "bedrock",
        "azure",
        "cohere",
        "groq",
        "qwen",
        "kimi",
        "xai",
        "ollama",
    ]
    .into_iter()
    .find(|provider| value.contains(provider))
    .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ScanOptions, scan_tools};

    #[test]
    fn discovers_multiple_formats_and_never_puts_values_in_preview() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        fs::create_dir_all(home.join(".codex")).unwrap();
        fs::create_dir_all(&project).unwrap();
        fs::write(
            home.join(".codex/settings.json"),
            r#"{"openai_api_key":"sk-json-secret","model":"gpt-test"}"#,
        )
        .unwrap();
        fs::write(
            home.join(".codex/config.toml"),
            "anthropic_api_key = \"sk-toml-secret\"\nmodels = [\"claude-test\"]\n",
        )
        .unwrap();
        fs::write(
            home.join(".codex/settings.yaml"),
            "google_api_key: sk-yaml-secret\ndefault_model: gemini-test\n",
        )
        .unwrap();
        fs::write(home.join(".codex/.env"), "MISTRAL_API_KEY=sk-env-secret\n").unwrap();

        let report = scan_tools(&ScanOptions::new(home, project).home_only());
        let result = discover_credentials(&report, &["codex".to_string()]);
        assert_eq!(result.vault.credentials.len(), 4);
        assert!(
            result
                .vault
                .models
                .iter()
                .any(|entry| entry.model == "gpt-test")
        );
        assert!(
            result
                .vault
                .models
                .iter()
                .any(|entry| entry.model == "claude-test")
        );

        let preview_json = serde_json::to_string(&result.preview).unwrap();
        assert!(!preview_json.contains("sk-json-secret"));
        assert!(!preview_json.contains("sk-toml-secret"));
        assert!(!preview_json.contains("sk-yaml-secret"));
        assert!(!preview_json.contains("sk-env-secret"));

        let output = temp.path().join("credentials.aibag");
        write_credential_vault(&result.vault, &output, "correct horse battery staple").unwrap();
        let encrypted = fs::read(&output).unwrap();
        assert!(!String::from_utf8_lossy(&encrypted).contains("sk-json-secret"));
        let decrypted =
            decrypt_credential_vault_bytes(&encrypted, "correct horse battery staple").unwrap();
        assert_eq!(decrypted.credentials.len(), 4);
    }
}
