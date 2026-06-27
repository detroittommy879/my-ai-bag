use crate::{
    preview::preview_for_selection_with_mode,
    scan::{Category, ScanReport, visit_limited},
};
use anyhow::{Context, Result, anyhow};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine, engine::general_purpose::STANDARD};
use chacha20poly1305::{
    Key, XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit},
};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct PackOptions {
    pub selected_keys: Vec<String>,
    pub mode: PackMode,
    pub output: PathBuf,
    pub passphrase: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum PackMode {
    #[default]
    Selective,
    AgentFolders,
}

impl PackMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Selective => "selective",
            Self::AgentFolders => "folders",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BagArchive {
    pub version: u32,
    pub created_by: String,
    pub home_dir: PathBuf,
    pub project_root: PathBuf,
    pub selected_tools: Vec<String>,
    #[serde(default)]
    pub mode: PackMode,
    pub files: Vec<BagFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BagFile {
    pub source_path: PathBuf,
    pub archive_path: String,
    pub category: Category,
    pub bytes_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptedEnvelope {
    version: u32,
    kdf: String,
    cipher: String,
    salt_base64: String,
    nonce_base64: String,
    payload_base64: String,
    warning: String,
}

pub fn pack_selected_tools(report: &ScanReport, options: &PackOptions) -> Result<BagArchive> {
    if options.passphrase.len() < 12 {
        return Err(anyhow!("passphrase must be at least 12 characters"));
    }

    let archive = build_archive(report, &options.selected_keys, options.mode)?;
    let payload = serde_json::to_vec_pretty(&archive)?;
    let encrypted = encrypt_payload(&payload, &options.passphrase)?;
    fs::write(&options.output, encrypted)
        .with_context(|| format!("failed to write {}", options.output.display()))?;

    Ok(archive)
}

pub fn decrypt_archive_bytes(encrypted: &[u8], passphrase: &str) -> Result<BagArchive> {
    Ok(serde_json::from_slice(&decrypt_payload(
        encrypted, passphrase,
    )?)?)
}

fn build_archive(
    report: &ScanReport,
    selected_keys: &[String],
    mode: PackMode,
) -> Result<BagArchive> {
    let selected: BTreeSet<&str> = selected_keys.iter().map(String::as_str).collect();
    let mut seen_files = BTreeSet::new();
    let mut files = Vec::new();

    for tool in report
        .tools
        .iter()
        .filter(|tool| selected.contains(tool.key.as_str()))
    {
        match mode {
            PackMode::Selective => {
                for found in &tool.found {
                    if found.is_dir {
                        collect_files_from_dir(
                            &found.path,
                            found.category,
                            &report.home_dir,
                            &report.project_root,
                            &mut seen_files,
                            &mut files,
                        )?;
                    } else {
                        push_file(
                            &found.path,
                            found.category,
                            &report.home_dir,
                            &report.project_root,
                            &mut seen_files,
                            &mut files,
                        )?;
                    }
                }
            }
            PackMode::AgentFolders => {
                for root in &tool.roots {
                    collect_files_from_dir(
                        &root.path,
                        Category::AgentFolder,
                        &report.home_dir,
                        &report.project_root,
                        &mut seen_files,
                        &mut files,
                    )?;
                }
            }
        }
    }

    let preview = preview_for_selection_with_mode(report, selected_keys, mode);
    if preview.unique_folders_to_include.is_empty() && files.is_empty() {
        return Err(anyhow!("nothing was found to pack for the selected tools"));
    }

    Ok(BagArchive {
        version: 1,
        created_by: "my-ai-bag-floem prototype".to_string(),
        home_dir: report.home_dir.clone(),
        project_root: report.project_root.clone(),
        selected_tools: selected_keys.to_vec(),
        mode,
        files,
    })
}

fn collect_files_from_dir(
    root: &Path,
    category: Category,
    home_dir: &Path,
    project_root: &Path,
    seen_files: &mut BTreeSet<PathBuf>,
    files: &mut Vec<BagFile>,
) -> Result<()> {
    let mut paths = Vec::new();
    visit_limited(root, 0, 12, &mut |path, is_dir| {
        if !is_dir {
            paths.push(path.to_path_buf());
        }
    });
    paths.sort();
    for path in paths {
        push_file(&path, category, home_dir, project_root, seen_files, files)?;
    }
    Ok(())
}

fn push_file(
    path: &Path,
    category: Category,
    home_dir: &Path,
    project_root: &Path,
    seen_files: &mut BTreeSet<PathBuf>,
    files: &mut Vec<BagFile>,
) -> Result<()> {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if !seen_files.insert(canonical) {
        return Ok(());
    }

    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    files.push(BagFile {
        source_path: path.to_path_buf(),
        archive_path: archive_path_for(path, home_dir, project_root),
        category,
        bytes_base64: STANDARD.encode(bytes),
    });
    Ok(())
}

fn archive_path_for(path: &Path, home_dir: &Path, project_root: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(home_dir) {
        format!("home/{}", slash_path(relative))
    } else if let Ok(relative) = path.strip_prefix(project_root) {
        format!("project/{}", slash_path(relative))
    } else {
        format!("absolute/{}", slash_path(path))
    }
}

fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn encrypt_payload(payload: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    let mut salt = [0_u8; 16];
    let mut nonce = [0_u8; 24];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);

    let key = derive_key(passphrase, &salt)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let encrypted = cipher
        .encrypt(XNonce::from_slice(&nonce), payload)
        .map_err(|_| anyhow!("failed to encrypt archive"))?;

    let envelope = EncryptedEnvelope {
        version: 1,
        kdf: "argon2id".to_string(),
        cipher: "xchacha20poly1305".to_string(),
        salt_base64: STANDARD.encode(salt),
        nonce_base64: STANDARD.encode(nonce),
        payload_base64: STANDARD.encode(encrypted),
        warning: "Encrypted local backup. Do not share this file unless you intend to share the secrets inside.".to_string(),
    };

    Ok(serde_json::to_vec_pretty(&envelope)?)
}

pub(crate) fn decrypt_payload(encrypted: &[u8], passphrase: &str) -> Result<Vec<u8>> {
    let envelope: EncryptedEnvelope = serde_json::from_slice(encrypted)?;
    let salt = STANDARD.decode(envelope.salt_base64)?;
    let nonce = STANDARD.decode(envelope.nonce_base64)?;
    let payload = STANDARD.decode(envelope.payload_base64)?;
    let key = derive_key(passphrase, &salt)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    cipher
        .decrypt(XNonce::from_slice(&nonce), payload.as_ref())
        .map_err(|_| anyhow!("failed to decrypt archive with the supplied passphrase"))
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let params = Params::new(19_456, 2, 1, Some(32))
        .map_err(|error| anyhow!("invalid kdf params: {error}"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0_u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|error| anyhow!("failed to derive encryption key: {error}"))?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use crate::{PackOptions, ScanOptions, scan_tools};
    use std::fs;

    use super::*;

    #[test]
    fn encrypted_pack_contains_files_but_not_plaintext_secret() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        fs::create_dir_all(home.join(".codex/skills/demo")).unwrap();
        fs::write(home.join(".codex/skills/demo/SKILL.md"), "demo skill").unwrap();
        fs::write(
            home.join(".codex/auth.json"),
            "{\"token\":\"sk-test-secret\"}",
        )
        .unwrap();
        fs::write(home.join(".codex/settings.json"), "{\"model\":\"demo\"}").unwrap();
        fs::create_dir_all(&project).unwrap();

        let report = scan_tools(&ScanOptions::new(home, project));
        let output = temp.path().join("bag.aibag");
        let archive = pack_selected_tools(
            &report,
            &PackOptions {
                selected_keys: vec!["codex".to_string()],
                mode: PackMode::Selective,
                output: output.clone(),
                passphrase: "correct horse battery staple".to_string(),
            },
        )
        .unwrap();

        assert!(
            archive
                .files
                .iter()
                .any(|file| file.archive_path.ends_with("auth.json"))
        );
        let encrypted = fs::read(&output).unwrap();
        let encrypted_text = String::from_utf8_lossy(&encrypted);
        assert!(!encrypted_text.contains("sk-test-secret"));

        let decrypted = decrypt_archive_bytes(&encrypted, "correct horse battery staple").unwrap();
        assert_eq!(decrypted.files.len(), archive.files.len());
    }

    #[test]
    fn folder_mode_packs_unclassified_files_and_uses_portable_paths() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        let project = temp.path().join("project");
        fs::create_dir_all(home.join(".codex/state/cache")).unwrap();
        fs::create_dir_all(&project).unwrap();
        fs::write(home.join(".codex/state/cache/custom.bin"), b"whole folder").unwrap();

        let report = scan_tools(&ScanOptions::new(home, project).home_only());
        let output = temp.path().join("folder-mode.aibag");
        let archive = pack_selected_tools(
            &report,
            &PackOptions {
                selected_keys: vec!["codex".to_string()],
                mode: PackMode::AgentFolders,
                output,
                passphrase: "correct horse battery staple".to_string(),
            },
        )
        .unwrap();

        assert!(archive.files.iter().any(|file| {
            file.archive_path == "home/.codex/state/cache/custom.bin"
                && file.category == Category::AgentFolder
        }));
    }
}
