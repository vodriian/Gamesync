//! Device credentials. No filesystem or plaintext fallback is permitted here.
use anyhow::{bail, Result};
use uuid::Uuid;

/// A real boundary for OS access and failure tests. Secrets never implement Debug.
pub trait CredentialStore {
    fn get(&self) -> Result<Option<String>>;
    fn set(&self, secret: &str) -> Result<()>;
    fn remove(&self) -> Result<()>;
}
/// One entry in the OS credential store.
pub struct OsCredential {
    entry: keyring::Entry,
}
impl OsCredential {
    /// The Steam Web API key of one local library.
    pub fn steam(library: Uuid) -> Result<Self> {
        Ok(Self {
            entry: keyring::Entry::new("app.GameSync.Steam", &library.to_string())?,
        })
    }
    /// The API key of one AI provider on this device, for every library.
    pub fn ai(provider: &str) -> Result<Self> {
        Ok(Self {
            entry: keyring::Entry::new("app.GameSync.AI", provider)?,
        })
    }
    /// The sync key of one sync folder. See `sync::secrets`.
    pub fn sync_key(sync_id: Uuid) -> Result<Self> {
        Ok(Self {
            entry: keyring::Entry::new("app.GameSync.SyncKey", &sync_id.to_string())?,
        })
    }
}
impl CredentialStore for OsCredential {
    fn get(&self) -> Result<Option<String>> {
        match self.entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => bail!("Secure storage is unavailable or locked. Unlock it and try again."),
        }
    }
    fn set(&self, secret: &str) -> Result<()> {
        self.entry.set_password(secret).map_err(|_| anyhow::anyhow!("Could not save the key in secure storage. The previous key was not deliberately removed."))
    }
    fn remove(&self) -> Result<()> {
        match self.entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => bail!("Could not remove the key. Unlock secure storage and try again."),
        }
    }
}
/// Test the replacement before touching the working credential.
pub fn replace_checked(
    store: &impl CredentialStore,
    key: &str,
    validate: impl FnOnce(&str) -> Result<()>,
) -> Result<()> {
    validate(key)?;
    // Saving the same tested connection twice must not rewrite the OS entry.
    if store.get()?.as_deref() == Some(key) {
        return Ok(());
    }
    store.set(key)
}
