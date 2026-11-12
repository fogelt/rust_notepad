//! API keys in the operating system's credential store.
//!
//! Keys are written to Windows Credential Manager, macOS Keychain, or the
//! Secret Service on Linux. They are never written to the config file and are
//! never shown again after saving.

use crate::app::ai::provider::Provider;

const SERVICE: &str = "rust_notepad";

fn entry_named(id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, &format!("api-key-{id}"))
        .map_err(|err| format!("Could not open the system keychain: {err}"))
}

fn entry(provider: Provider) -> Result<keyring::Entry, String> {
    entry_named(provider.id())
}

/// Read a stored key. Missing credentials and keychain failures both return
/// `None`; callers treat that as "no key configured".
pub fn load(provider: Provider) -> Option<String> {
    load_named(provider.id())
}

/// Read a key by its raw keychain id. Used once to migrate legacy providers.
pub(crate) fn load_named(id: &str) -> Option<String> {
    let entry = entry_named(id).ok()?;
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
    delete_named(provider.id())
}

/// Remove a key by its raw keychain id.
pub(crate) fn delete_named(id: &str) -> Result<(), String> {
    match entry_named(id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(format!(
            "Could not remove the key from the system keychain: {err}"
        )),
    }
}
