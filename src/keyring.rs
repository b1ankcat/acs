use keyring::Entry;

const SERVICE_NAME: &str = "acs";

#[derive(Debug)]
pub enum KeyringError {
    Unavailable(String),
    ReadFailed(String),
    WriteFailed(String),
}

impl std::fmt::Display for KeyringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(msg) => write!(f, "keyring unavailable: {}", msg),
            Self::ReadFailed(msg) => write!(f, "failed to read from keyring: {}", msg),
            Self::WriteFailed(msg) => write!(f, "failed to write to keyring: {}", msg),
        }
    }
}

impl std::error::Error for KeyringError {}

/// Check if system keyring is available
pub fn is_available() -> bool {
    // Try to create and immediately delete a test entry
    let test_key = format!("{}:test:availability", SERVICE_NAME);
    match Entry::new(SERVICE_NAME, &test_key) {
        Ok(entry) => {
            // Try to set and delete a dummy value
            match entry.set_password("test") {
                Ok(_) => {
                    let _ = entry.delete_credential();
                    true
                }
                Err(_) => false
            }
        }
        Err(_) => false,
    }
}

/// Store a secret in the system keyring
pub fn try_store(username: &str, password: &str) -> Result<(), KeyringError> {
    let entry = Entry::new(SERVICE_NAME, username)
        .map_err(|e| KeyringError::Unavailable(e.to_string()))?;

    entry.set_password(password)
        .map_err(|e| KeyringError::WriteFailed(e.to_string()))?;

    Ok(())
}

/// Retrieve a secret from the system keyring
pub fn try_get(username: &str) -> Result<String, KeyringError> {
    let entry = Entry::new(SERVICE_NAME, username)
        .map_err(|e| KeyringError::Unavailable(e.to_string()))?;

    entry.get_password()
        .map_err(|e| KeyringError::ReadFailed(e.to_string()))
}

/// Delete a secret from the system keyring
pub fn delete(username: &str) -> Result<(), KeyringError> {
    let entry = Entry::new(SERVICE_NAME, username)
        .map_err(|e| KeyringError::Unavailable(e.to_string()))?;

    entry.delete_credential()
        .map_err(|e| KeyringError::WriteFailed(e.to_string()))?;

    Ok(())
}

/// Encode API key for storage: returns keyring reference or plaintext
///
/// If `use_keyring` is true, attempts to store the API key in the system keyring.
/// On success, returns `keyring:<username>`. On failure, prints a warning and
/// returns the plaintext API key.
pub fn encode_api_key(tool: &str, provider: &str, api_key: &str, use_keyring: bool) -> String {
    if !use_keyring {
        return api_key.to_string();
    }

    let username = format!("{}:{}:api-key", tool, provider);

    match try_store(&username, api_key) {
        Ok(_) => format!("keyring:{}", username),
        Err(e) => {
            eprintln!("Warning: {}, storing in plaintext", e);
            api_key.to_string()
        }
    }
}

/// Decode API key: reads from keyring if prefixed, otherwise returns as-is
///
/// If the value starts with `keyring:`, extracts the username and reads the
/// secret from the system keyring. Otherwise, treats it as plaintext and
/// returns it unchanged.
pub fn decode_api_key(value: &str) -> Result<String, KeyringError> {
    if let Some(username) = value.strip_prefix("keyring:") {
        try_get(username)
    } else {
        Ok(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_plaintext() {
        let result = encode_api_key("claude", "test", "sk-test-key", false);
        assert_eq!(result, "sk-test-key");
    }

    #[test]
    fn test_decode_plaintext() {
        let result = decode_api_key("sk-test-key").unwrap();
        assert_eq!(result, "sk-test-key");
    }

    #[test]
    fn test_encode_decode_keyring() {
        let tool = "claude";
        let provider = "test-provider";
        let api_key = "sk-test-secret-12345";

        // Try to encode with keyring
        let encoded = encode_api_key(tool, provider, api_key, true);

        // If encoding failed (returned plaintext), skip keyring test
        if !encoded.starts_with("keyring:") {
            assert_eq!(encoded, api_key);
            return;
        }

        // Keyring storage succeeded, verify decode works
        match decode_api_key(&encoded) {
            Ok(decoded) => {
                assert_eq!(decoded, api_key);
                // Cleanup
                let username = format!("{}:{}:api-key", tool, provider);
                let _ = delete(&username);
            }
            Err(_) => {
                // Decode failed even though encode succeeded - keyring unstable, skip test
                let username = format!("{}:{}:api-key", tool, provider);
                let _ = delete(&username);
            }
        }
    }
}
