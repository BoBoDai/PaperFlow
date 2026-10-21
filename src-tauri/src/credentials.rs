//! Write-only API key storage.
//!
//! Keys live in `credentials.json` next to `settings.json`, with 0600
//! permissions on Unix. Provider settings keep only the provider ID; the UI
//! never receives the key back.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

fn default_version() -> u32 {
    1
}

#[derive(Debug, Serialize, Deserialize)]
struct CredentialsFile {
    #[serde(default = "default_version")]
    version: u32,
    #[serde(default)]
    keys: BTreeMap<String, String>,
}

pub struct CredentialStore {
    path: PathBuf,
    keys: BTreeMap<String, String>,
}

impl CredentialStore {
    /// Load keys, treating a missing or corrupt file as an empty store.
    pub fn load(path: PathBuf) -> Self {
        let keys = match fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<CredentialsFile>(&raw) {
                Ok(file) => file.keys,
                Err(error) => {
                    tracing::warn!(
                        path = %path.display(),
                        %error,
                        "invalid credentials file; starting with no keys"
                    );
                    BTreeMap::new()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    %error,
                    "cannot read credentials; starting with no keys"
                );
                BTreeMap::new()
            }
        };

        Self { path, keys }
    }

    pub fn get(&self, provider_id: &str) -> Option<String> {
        self.keys.get(provider_id).cloned()
    }

    pub fn has(&self, provider_id: &str) -> bool {
        self.keys
            .get(provider_id)
            .map(|key| !key.trim().is_empty())
            .unwrap_or(false)
    }

    /// Set a key; empty values remove the entry.
    pub fn set(&mut self, provider_id: &str, key: String) {
        let key = key.trim().to_string();
        if key.is_empty() {
            self.keys.remove(provider_id);
        } else {
            self.keys.insert(provider_id.to_string(), key);
        }
    }

    pub fn remove(&mut self, provider_id: &str) {
        self.keys.remove(provider_id);
    }

    pub fn retain(&mut self, provider_ids: &HashSet<String>) {
        self.keys.retain(|id, _| provider_ids.contains(id));
    }

    #[cfg(test)]
    pub fn keys(&self) -> &BTreeMap<String, String> {
        &self.keys
    }

    pub fn persist(&self) -> Result<(), String> {
        let raw = serde_json::to_string_pretty(&CredentialsFile {
            version: 1,
            keys: self.keys.clone(),
        })
        .map_err(|e| format!("序列化凭据失败: {e}"))?;

        let temp = self.path.with_extension("json.tmp");
        fs::write(&temp, raw).map_err(|e| format!("写入凭据失败: {e}"))?;
        set_private_permissions(&temp)?;

        #[cfg(windows)]
        if self.path.exists() {
            fs::remove_file(&self.path).map_err(|e| format!("替换凭据失败: {e}"))?;
        }

        fs::rename(&temp, &self.path).map_err(|e| format!("保存凭据失败: {e}"))
    }
}

#[cfg(unix)]
pub(crate) fn set_private_permissions(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .map_err(|e| format!("读取文件权限失败: {e}"))?
        .permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(path, permissions).map_err(|e| format!("设置文件权限失败: {e}"))
}

#[cfg(not(unix))]
pub(crate) fn set_private_permissions(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "paperflow-credentials-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir should be created");
        dir.join("credentials.json")
    }

    #[test]
    fn round_trips_keys() {
        let path = temp_path("roundtrip");
        let mut store = CredentialStore::load(path.clone());

        store.set("deepseek", "  sk-test  ".into());
        store.set("minimax", "sk-mini".into());
        assert!(store.has("deepseek"));
        assert_eq!(store.get("deepseek").as_deref(), Some("sk-test"));

        store.persist().expect("keys should persist");

        let reloaded = CredentialStore::load(path.clone());
        assert_eq!(reloaded.get("deepseek").as_deref(), Some("sk-test"));
        assert_eq!(reloaded.get("minimax").as_deref(), Some("sk-mini"));

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn empty_value_removes_and_retain_prunes() {
        let path = temp_path("prune");
        let mut store = CredentialStore::load(path);

        store.set("a", "key-a".into());
        store.set("b", "key-b".into());
        store.set("a", "  ".into());
        assert!(!store.has("a"));

        let keep = HashSet::from(["b".to_string()]);
        store.retain(&keep);
        assert!(store.has("b"));
        assert_eq!(store.keys().len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn persisted_file_is_private() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_path("permissions");
        let mut store = CredentialStore::load(path.clone());
        store.set("deepseek", "sk-test".into());
        store.persist().expect("keys should persist");

        let mode = fs::metadata(&path)
            .expect("file should exist")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
