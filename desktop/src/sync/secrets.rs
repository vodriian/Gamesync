//! Encrypted API keys in the sync folder. See `plan/data-sync.md`.
//!
//! A random sync key (an age X25519 identity) encrypts each API key. The
//! passphrase encrypts only the sync key, once, because passphrase
//! encryption is slow on purpose. Each device keeps the sync key in its OS
//! credential store after the user enters the passphrase once.
//!
//! A sealed value also has a fingerprint made with the sync key. Devices
//! compare fingerprints, because the same key encrypts to new bytes each time.

pub use age::secrecy::{ExposeSecret, SecretString};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// The synced target that holds the passphrase-protected sync key.
pub const SYNC_KEY: &str = "sync_key";
/// The field of `SYNC_KEY`; also the field of each sealed API key.
pub const WRAPPED_FIELD: &str = "wrapped";
pub const VALUE_FIELD: &str = "value";
/// Short passphrases are easy to guess offline from a copy of the folder.
pub const MIN_PASSPHRASE_LEN: usize = 10;
/// Limits decryption work for a folder value from another writer.
const MAX_CIPHERTEXT_LEN: usize = 64 << 10;

pub struct SyncKey {
    identity: age::x25519::Identity,
}

impl SyncKey {
    pub fn generate() -> Self {
        Self {
            identity: age::x25519::Identity::generate(),
        }
    }

    /// The form kept in the OS credential store.
    pub fn to_secret(&self) -> SecretString {
        self.identity.to_string()
    }

    pub fn from_secret(text: &str) -> Result<Self> {
        Ok(Self {
            identity: text
                .parse()
                .map_err(|_| anyhow::anyhow!("The saved sync key is not valid"))?,
        })
    }

    /// Encrypt the sync key with the passphrase, for the sync folder.
    pub fn wrap(&self, passphrase: &str) -> Result<Value> {
        self.wrap_with(passphrase, None)
    }

    fn wrap_with(&self, passphrase: &str, work_factor: Option<u8>) -> Result<Value> {
        ensure!(
            passphrase.chars().count() >= MIN_PASSPHRASE_LEN,
            "Use a passphrase with at least {MIN_PASSPHRASE_LEN} characters"
        );
        let mut recipient = age::scrypt::Recipient::new(SecretString::from(passphrase.to_owned()));
        if let Some(factor) = work_factor {
            recipient.set_work_factor(factor);
        }
        let armored =
            age::encrypt_and_armor(&recipient, self.to_secret().expose_secret().as_bytes())
                .context("Could not protect the sync key")?;
        // The public recipient lets a device see that the sync key was
        // replaced on another device without the passphrase.
        Ok(json!({ "age": armored, "recipient": self.recipient() }))
    }

    pub fn recipient(&self) -> String {
        self.identity.to_public().to_string()
    }

    /// True when `wrapped` protects this sync key.
    pub fn matches(&self, wrapped: &Value) -> bool {
        wrapped.get("recipient").and_then(Value::as_str) == Some(self.recipient().as_str())
    }

    /// Decrypt the sync key from the folder. A wrong passphrase fails here,
    /// before anything is written.
    pub fn unwrap(wrapped: &Value, passphrase: &str) -> Result<Self> {
        let armored = ciphertext(wrapped)?;
        let identity = age::scrypt::Identity::new(SecretString::from(passphrase.to_owned()));
        let bytes = age::decrypt(&identity, armored.as_bytes())
            .map_err(|_| anyhow::anyhow!("The passphrase is wrong."))?;
        let text = String::from_utf8(bytes).context("The sync key is not valid")?;
        Self::from_secret(&text)
    }

    /// Encrypt an API key for the folder, with a fingerprint to compare.
    pub fn seal(&self, name: &str, secret: &str) -> Result<Value> {
        let armored = age::encrypt_and_armor(&self.identity.to_public(), secret.as_bytes())
            .context("Could not encrypt the key")?;
        Ok(json!({ "fp": self.fingerprint(name, secret), "age": armored }))
    }

    pub fn open(&self, sealed: &Value) -> Result<SecretString> {
        let bytes = age::decrypt(&self.identity, ciphertext(sealed)?.as_bytes())
            .map_err(|_| anyhow::anyhow!("The key was encrypted with another sync key"))?;
        Ok(SecretString::from(
            String::from_utf8(bytes).context("The key is not valid text")?,
        ))
    }

    /// A keyed hash: without the sync key it tells nothing about the API key.
    fn fingerprint(&self, name: &str, secret: &str) -> String {
        let mut hash = Sha256::new();
        for part in [self.to_secret().expose_secret(), name, secret] {
            hash.update((part.len() as u64).to_be_bytes());
            hash.update(part.as_bytes());
        }
        hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }
}

fn ciphertext(value: &Value) -> Result<&str> {
    let text = value
        .get("age")
        .and_then(Value::as_str)
        .context("The protected value is missing")?;
    ensure!(
        text.len() <= MAX_CIPHERTEXT_LEN,
        "The protected value is too large"
    );
    Ok(text)
}

/// Two sealed values hold the same key when their fingerprints match. The
/// engine compares with this, because each encryption gives new bytes.
pub fn same_value(a: &Value, b: &Value) -> bool {
    match (a.get("fp"), b.get("fp")) {
        (Some(a), Some(b)) => a == b,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The lowest work factor keeps tests fast; the app uses age's default.
    const FAST: Option<u8> = Some(1);

    #[test]
    fn the_right_passphrase_opens_the_sync_key() {
        let key = SyncKey::generate();
        let wrapped = key.wrap_with("correct horse battery", FAST).unwrap();
        let opened = SyncKey::unwrap(&wrapped, "correct horse battery").unwrap();
        assert!(opened.matches(&wrapped));
        assert!(!SyncKey::generate().matches(&wrapped));
        assert_eq!(
            opened.to_secret().expose_secret(),
            key.to_secret().expose_secret()
        );
        let error = SyncKey::unwrap(&wrapped, "wrong horse battery")
            .err()
            .unwrap();
        assert_eq!(error.to_string(), "The passphrase is wrong.");
        assert!(key.wrap_with("short", FAST).is_err());
    }

    #[test]
    fn sealed_keys_open_and_compare_by_fingerprint() {
        let key = SyncKey::generate();
        let first = key.seal("steam_api_key", "ABC123").unwrap();
        let second = key.seal("steam_api_key", "ABC123").unwrap();
        assert_ne!(first["age"], second["age"], "encryption is randomized");
        assert!(same_value(&first, &second));
        assert!(!same_value(
            &first,
            &key.seal("steam_api_key", "XYZ").unwrap()
        ));
        assert!(!same_value(
            &first,
            &key.seal("ai.openai", "ABC123").unwrap()
        ));
        assert_eq!(key.open(&first).unwrap().expose_secret(), "ABC123");

        // Another sync key cannot open it, and its fingerprints differ.
        let other = SyncKey::generate();
        assert!(other.open(&first).is_err());
        assert!(!same_value(
            &first,
            &other.seal("steam_api_key", "ABC123").unwrap()
        ));
        assert!(!first.to_string().contains("ABC123"));
    }
}
