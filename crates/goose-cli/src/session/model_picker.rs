use anyhow::Result;
use goose::providers::inventory::{ProviderInventoryEntry, ProviderInventoryService};
use goose::session::SessionManager;
use std::io;

const MAX_ROWS: usize = 10;
const UNLISTED_MODEL: &str = "__unlisted__";

/// Returns `None` when the user cancels with Esc or Ctrl-C.
pub async fn select_provider(current: &str) -> Result<Option<String>> {
    let storage = SessionManager::instance().storage().clone();
    let entries = ProviderInventoryService::new(storage).entries(&[]).await?;
    let items: Vec<(String, String, &str)> = provider_choices(entries, current)
        .into_iter()
        .map(|(id, name)| {
            let label = mark_current(&name, id == current);
            (id, label, "")
        })
        .collect();
    cancellable(
        cliclack::select("Select a provider")
            .items(&items)
            .initial_value(current.to_string())
            .max_rows(MAX_ROWS)
            .interact(),
    )
}

/// Returns `None` when the user cancels with Esc or Ctrl-C.
pub fn select_model(
    provider: &str,
    models: &[String],
    current: Option<&str>,
) -> Result<Option<String>> {
    let mut items: Vec<(String, String, &str)> = model_choices(models, current)
        .into_iter()
        .map(|model| {
            let label = mark_current(&model, Some(model.as_str()) == current);
            (model, label, "")
        })
        .collect();
    items.push((
        UNLISTED_MODEL.to_string(),
        "Enter a model not listed…".to_string(),
        "",
    ));

    let mut select = cliclack::select(format!("Select a model ({provider})"))
        .items(&items)
        .max_rows(MAX_ROWS);
    if let Some(current) = current {
        select = select.initial_value(current.to_string());
    }
    match cancellable(select.interact())? {
        Some(choice) if choice == UNLISTED_MODEL => cancellable(
            cliclack::input("Model name")
                .validate(|input: &String| {
                    if input.trim().is_empty() {
                        Err("Please enter a model name")
                    } else {
                        Ok(())
                    }
                })
                .interact::<String>()
                .map(|model| model.trim().to_string()),
        ),
        choice => Ok(choice),
    }
}

fn provider_choices(entries: Vec<ProviderInventoryEntry>, current: &str) -> Vec<(String, String)> {
    let mut choices: Vec<(String, String)> = entries
        .into_iter()
        .filter(|entry| !entry.acp && (entry.configured || entry.provider_id == current))
        .map(|entry| (entry.provider_id, entry.provider_name))
        .collect();
    choices.sort_by_key(|(_, name)| name.to_lowercase());
    choices
}

fn model_choices(models: &[String], current: Option<&str>) -> Vec<String> {
    let mut choices = models.to_vec();
    if let Some(current) = current.filter(|current| !models.iter().any(|m| m == current)) {
        choices.insert(0, current.to_string());
    }
    choices
}

fn mark_current(label: &str, is_current: bool) -> String {
    if is_current {
        format!("{label} (current)")
    } else {
        label.to_string()
    }
}

fn cancellable<T>(result: io::Result<T>) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(e) if e.kind() == io::ErrorKind::Interrupted => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use goose::providers::base::ProviderType;

    fn entry(id: &str, name: &str, configured: bool, acp: bool) -> ProviderInventoryEntry {
        ProviderInventoryEntry {
            provider_id: id.to_string(),
            provider_name: name.to_string(),
            description: String::new(),
            default_model: String::new(),
            configured,
            available: configured,
            provider_type: ProviderType::Builtin,
            acp,
            visible_in_setup: true,
            deprecated: false,
            replacement: None,
            config_keys: Vec::new(),
            setup_steps: Vec::new(),
            supports_refresh: false,
            refreshing: false,
            models: Vec::new(),
            last_updated_at: None,
            last_refresh_attempt_at: None,
            last_refresh_error: None,
        }
    }

    #[test]
    fn provider_choices_lists_configured_and_current_without_acp_sorted_by_name() {
        let choices = provider_choices(
            vec![
                entry("openai", "OpenAI", true, false),
                entry("claude-acp", "Claude Code", true, true),
                entry("groq", "Groq", false, false),
                entry("ollama", "ollama", false, false),
                entry("anthropic", "Anthropic", true, false),
            ],
            "ollama",
        );
        let ids: Vec<&str> = choices.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["anthropic", "ollama", "openai"]);
    }

    #[test]
    fn model_choices_prepends_current_model_missing_from_list() {
        let models = vec!["gpt-4o".to_string(), "o3".to_string()];
        assert_eq!(
            model_choices(&models, Some("my-finetune")),
            ["my-finetune", "gpt-4o", "o3"]
        );
    }

    #[test]
    fn model_choices_keeps_order_when_current_is_listed() {
        let models = vec!["gpt-4o".to_string(), "o3".to_string()];
        assert_eq!(model_choices(&models, Some("o3")), ["gpt-4o", "o3"]);
        assert_eq!(model_choices(&models, None), ["gpt-4o", "o3"]);
    }
}
