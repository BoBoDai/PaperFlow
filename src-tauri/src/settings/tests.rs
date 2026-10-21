use super::*;
use std::fs;
use std::path::PathBuf;

use paperflow_adapters::{LlmCompat, ProviderProfileId};

fn temp_dir(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("paperflow-settings-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}

fn provider(
    id: &str,
    name: &str,
    base_url: &str,
    model: &str,
    provider: ProviderProfileId,
) -> LlmProviderInput {
    LlmProviderInput {
        id: id.into(),
        name: name.into(),
        base_url: base_url.into(),
        model: model.into(),
        provider,
        compat: LlmCompat::default(),
        api_key: None,
        clear_api_key: false,
    }
}

#[test]
fn defaults_to_one_active_deepseek_provider() {
    let settings = LlmSettings::default();
    assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
    assert_eq!(settings.providers.len(), 1);
    assert_eq!(settings.active_id, settings.providers[0].id);
    assert_eq!(settings.providers[0].provider, ProviderProfileId::DeepSeek);
}

#[test]
fn saves_redacted_view_and_separate_credentials() {
    let dir = temp_dir("roundtrip");
    let manager = SettingsManager::load(dir.clone());

    let mut first = provider(
        "deepseek",
        "DeepSeek 官方",
        "https://api.deepseek.com/",
        "deepseek-chat",
        ProviderProfileId::DeepSeek,
    );
    first.api_key = Some("sk-deepseek".into());

    let mut second = provider(
        "minimax",
        "MiniMax",
        "https://api.minimaxi.com/v1",
        "MiniMax-M2",
        ProviderProfileId::MiniMax,
    );
    second.api_key = Some("sk-minimax".into());

    let saved = manager
        .save(&LlmSettingsInput {
            active_id: "minimax".into(),
            providers: vec![first, second],
        })
        .expect("settings should save");

    assert_eq!(saved.active_id, "minimax");
    assert!(saved.providers.iter().all(|provider| provider.has_api_key));
    assert!(!serde_json::to_string(&saved).unwrap().contains("sk-"));

    let settings_raw = fs::read_to_string(dir.join("settings.json")).unwrap();
    assert!(!settings_raw.contains("sk-"));
    let credentials_raw = fs::read_to_string(dir.join("credentials.json")).unwrap();
    assert!(credentials_raw.contains("sk-deepseek"));

    let reloaded = SettingsManager::load(dir.clone());
    let view = reloaded.current_view();
    assert_eq!(view.providers.len(), 2);
    assert!(view.providers.iter().all(|provider| provider.has_api_key));
    assert_eq!(reloaded.shared().read().unwrap().model, "MiniMax-M2");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn blank_key_keeps_existing_and_clear_removes() {
    let dir = temp_dir("keys");
    let manager = SettingsManager::load(dir.clone());

    let mut provider = provider(
        "deepseek",
        "DeepSeek 官方",
        "https://api.deepseek.com",
        "deepseek-chat",
        ProviderProfileId::DeepSeek,
    );
    provider.api_key = Some("sk-test".into());
    manager
        .save(&LlmSettingsInput {
            active_id: "deepseek".into(),
            providers: vec![provider.clone()],
        })
        .expect("settings should save");

    provider.api_key = None;
    let saved = manager
        .save(&LlmSettingsInput {
            active_id: "deepseek".into(),
            providers: vec![provider.clone()],
        })
        .expect("blank key should keep existing");
    assert!(saved.providers[0].has_api_key);

    provider.clear_api_key = true;
    let saved = manager
        .save(&LlmSettingsInput {
            active_id: "deepseek".into(),
            providers: vec![provider],
        })
        .expect("clear should remove");
    assert!(!saved.providers[0].has_api_key);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn deleting_a_provider_prunes_its_key() {
    let dir = temp_dir("prune");
    let manager = SettingsManager::load(dir.clone());

    let mut first = provider(
        "a",
        "A",
        "https://a.example/v1",
        "model-a",
        ProviderProfileId::Custom,
    );
    first.api_key = Some("key-a".into());
    let mut second = provider(
        "b",
        "B",
        "https://b.example/v1",
        "model-b",
        ProviderProfileId::Custom,
    );
    second.api_key = Some("key-b".into());

    manager
        .save(&LlmSettingsInput {
            active_id: "a".into(),
            providers: vec![first, second],
        })
        .expect("settings should save");

    let first = provider(
        "a",
        "A",
        "https://a.example/v1",
        "model-a",
        ProviderProfileId::Custom,
    );
    manager
        .save(&LlmSettingsInput {
            active_id: "a".into(),
            providers: vec![first],
        })
        .expect("settings should save");

    let credentials = fs::read_to_string(dir.join("credentials.json")).unwrap();
    assert!(credentials.contains("key-a"));
    assert!(!credentials.contains("key-b"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn migrates_legacy_single_provider_with_key() {
    let dir = temp_dir("legacy");
    fs::write(
        dir.join("settings.json"),
        r#"{"api_key":"sk-old","base_url":"https://api.deepseek.com/","model":"deepseek-chat"}"#,
    )
    .unwrap();

    let manager = SettingsManager::load(dir.clone());
    let view = manager.current_view();

    assert_eq!(view.providers.len(), 1);
    assert_eq!(view.providers[0].provider, ProviderProfileId::DeepSeek);
    assert!(view.providers[0].has_api_key);

    let settings_raw = fs::read_to_string(dir.join("settings.json")).unwrap();
    assert!(!settings_raw.contains("sk-old"));
    let credentials_raw = fs::read_to_string(dir.join("credentials.json")).unwrap();
    assert!(credentials_raw.contains("sk-old"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn migrates_current_format_and_keeps_custom_gateways_custom() {
    let dir = temp_dir("current");
    fs::write(
        dir.join("settings.json"),
        r#"{
          "active_id": "gateway",
          "providers": [
            {"id":"gateway","name":"Gateway","base_url":"https://gateway.example/v1","model":"custom-chat","provider":"openai","api_key":"sk-gateway"},
            {"id":"mini","name":"MiniMax","base_url":"https://api.minimaxi.com/v1","model":"MiniMax-M2","provider":"minimax","api_key":"sk-mini"}
          ]
        }"#,
    )
    .unwrap();

    let manager = SettingsManager::load(dir.clone());
    let view = manager.current_view();

    assert_eq!(view.providers[0].provider, ProviderProfileId::Custom);
    assert_eq!(view.providers[1].provider, ProviderProfileId::MiniMax);
    assert!(view.providers.iter().all(|provider| provider.has_api_key));

    let settings_raw = fs::read_to_string(dir.join("settings.json")).unwrap();
    assert!(!settings_raw.contains("sk-gateway"));
    assert!(!settings_raw.contains("sk-mini"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn rejects_invalid_base_url_and_duplicate_ids() {
    let dir = temp_dir("validation");
    let manager = SettingsManager::load(dir.clone());

    let invalid = provider(
        "a",
        "A",
        "api.example.com",
        "model",
        ProviderProfileId::Custom,
    );
    assert!(
        manager
            .save(&LlmSettingsInput {
                active_id: "a".into(),
                providers: vec![invalid],
            })
            .is_err()
    );

    let duplicate_a = provider(
        "a",
        "A",
        "https://a.example/v1",
        "model",
        ProviderProfileId::Custom,
    );
    let duplicate_b = provider(
        "a",
        "B",
        "https://b.example/v1",
        "model",
        ProviderProfileId::Custom,
    );
    assert!(
        manager
            .save(&LlmSettingsInput {
                active_id: "a".into(),
                providers: vec![duplicate_a, duplicate_b],
            })
            .is_err()
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn stale_active_id_falls_back_to_first_provider() {
    let dir = temp_dir("stale");
    fs::write(
        dir.join("settings.json"),
        r#"{"schema_version":2,"active_id":"missing","providers":[{"id":"a","name":"A","base_url":"https://a.example/v1","model":"m","provider":"custom"}]}"#,
    )
    .unwrap();

    let manager = SettingsManager::load(dir.clone());
    assert_eq!(manager.current_view().active_id, "a");

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn corrupt_credentials_do_not_break_settings() {
    let dir = temp_dir("corrupt-credentials");
    fs::write(dir.join("credentials.json"), "{not json").unwrap();

    let manager = SettingsManager::load(dir.clone());
    assert!(!manager.current_view().providers[0].has_api_key);

    let _ = fs::remove_dir_all(dir);
}
