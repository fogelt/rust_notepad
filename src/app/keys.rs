//! API keys in the operating system's credential store.
//!
//! Keys are written to Windows Credential Manager, macOS Keychain, or the
//! Secret Service on Linux. They are never written to the config file and are
//! never shown again after saving.

use crate::app::ai::provider::Provider;

const SERVICE: &str = "rust_notepad";

fn entry(provider: Provider) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, &format!("api-key-{}", provider.id()))
        .map_err(|err| format!("Could not open the system keychain: {err}"))
}

/// Read a stored key. Missing credentials and keychain failures both return
/// `None`; callers treat that as "no key configured".
pub fn load(provider: Provider) -> Option<String> {
    let entry = entry(provider).ok()?;
    match entry.get_password() {
        Ok(key) if !key.trim().is_empty() => Some(key),
        Ok(_) => None,
        Err(keyring::Error::NoEntry) => None,
        Err(_) => None,
    }
}

pub fn has(provider: Provider) -> bool {
    load(provider).is_some()
}

pub fn store(provider: Provider, key: &str) -> Result<(), String> {
    let entry = entry(provider)?;
    entry
        .set_password(key.trim())
        .map_err(|err| format!("Could not save the key in the system keychain: {err}"))
}

pub fn delete(provider: Provider) -> Result<(), String> {
    match entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(format!(
            "Could not remove the key from the system keychain: {err}"
        )),
    }
}
