use crate::config::{self, AcsConfig, Provider, ToolConfig};
use crate::errors::{AcsError, ProviderError};
use crate::keyring;
use std::collections::HashMap;

/// Add or update a provider in a tool's configuration
pub fn add_or_update(
    tool_name: &str,
    cfg: &mut AcsConfig,
    provider_name: &str,
    fields: &HashMap<&str, &str>,
    fallback_urls: &[String],
    use_keyring: bool,
    no_keyring: bool,
) -> Result<(Provider, bool), AcsError> {
    config::validate_provider_name(provider_name)?;

    let tool = cfg.get_tool_mut(tool_name);
    let was_new = !tool.providers.contains_key(provider_name);

    // Process API key encryption if provided
    let mut updated_fields: HashMap<String, String> = fields
        .iter()
        .map(|(&k, &v)| (k.to_string(), v.to_string()))
        .collect();

    if let Some(api_key) = updated_fields.get("api-key") {
        let should_use_keyring = if no_keyring {
            false
        } else {
            use_keyring || (!was_new && is_keyring_reference(tool.providers.get(provider_name)))
        };

        let encoded = keyring::encode_api_key(tool_name, provider_name, api_key, should_use_keyring);
        updated_fields.insert("api-key".to_string(), encoded);
    }

    // Merge with existing provider fields if updating
    let base_fields = if was_new {
        HashMap::new()
    } else {
        tool.providers[provider_name].fields.clone()
    };

    let merged_fields: HashMap<String, String> = base_fields
        .into_iter()
        .chain(updated_fields)
        .collect();

    // Merge fallback URLs
    let existing_fallbacks = if was_new {
        vec![]
    } else {
        tool.providers[provider_name].fallback_urls.clone()
    };
    let all_fallbacks = config::merge_fallback_urls(existing_fallbacks, fallback_urls);

    let provider = Provider {
        fields: merged_fields,
        fallback_urls: all_fallbacks,
    };

    tool.providers.insert(provider_name.to_string(), provider.clone());

    Ok((provider, was_new))
}

/// Remove a provider from a tool's configuration
pub fn remove(
    tool_name: &str,
    cfg: &mut AcsConfig,
    provider_name: &str,
) -> Result<(), AcsError> {
    let tool = cfg.get_tool_mut(tool_name);

    if !tool.providers.contains_key(provider_name) {
        return Err(ProviderError::not_found(provider_name, tool_name).into());
    }

    if tool.active == provider_name {
        return Err(ProviderError::no_removable(tool_name).into());
    }

    tool.providers.remove(provider_name);
    Ok(())
}

/// Check if a provider's API key is stored in keyring
fn is_keyring_reference(provider: Option<&Provider>) -> bool {
    provider
        .and_then(|p| p.fields.get("api-key"))
        .map(|k| k.starts_with("keyring:"))
        .unwrap_or(false)
}

/// Set active provider and apply configuration
pub fn set_active(
    tool_name: &str,
    cfg: &mut AcsConfig,
    provider_name: &str,
) -> Result<Provider, AcsError> {
    let tool = cfg.get_tool(tool_name);

    if !tool.providers.contains_key(provider_name) {
        return Err(ProviderError::not_found(provider_name, tool_name).into());
    }

    let provider = tool.providers[provider_name].clone();

    cfg.get_tool_mut(tool_name).active = provider_name.to_string();

    Ok(provider)
}

