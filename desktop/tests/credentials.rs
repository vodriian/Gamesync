use anyhow::Result;
use gamesync_desktop::credentials::{replace_checked, CredentialStore};
use std::cell::{Cell, RefCell};
struct Store {
    value: RefCell<Option<String>>,
    fail: bool,
    writes: Cell<usize>,
}
impl CredentialStore for Store {
    fn get(&self) -> Result<Option<String>> {
        Ok(self.value.borrow().clone())
    }
    fn set(&self, key: &str) -> Result<()> {
        anyhow::ensure!(!self.fail, "Locked");
        self.writes.set(self.writes.get() + 1);
        *self.value.borrow_mut() = Some(key.into());
        Ok(())
    }
    fn remove(&self) -> Result<()> {
        *self.value.borrow_mut() = None;
        Ok(())
    }
}
#[test]
fn failed_validation_or_locked_store_keeps_previous_key() {
    let store = Store {
        value: RefCell::new(Some("old-dummy".into())),
        fail: false,
        writes: Cell::new(0),
    };
    assert!(replace_checked(&store, "new-dummy", |_| anyhow::bail!("Rejected")).is_err());
    assert_eq!(store.get().unwrap().as_deref(), Some("old-dummy"));
    let locked = Store {
        fail: true,
        ..store
    };
    assert!(replace_checked(&locked, "new-dummy", |_| Ok(())).is_err());
    assert_eq!(locked.get().unwrap().as_deref(), Some("old-dummy"));
}
#[test]
fn saving_a_tested_key_twice_writes_once() {
    let store = Store {
        value: RefCell::new(None),
        fail: false,
        writes: Cell::new(0),
    };
    replace_checked(&store, "dummy", |_| Ok(())).unwrap();
    replace_checked(&store, "dummy", |_| Ok(())).unwrap();
    assert_eq!(store.writes.get(), 1);
}
#[test]
#[ignore = "Uses OS secure storage with a disposable UUID and dummy value"]
fn native_store_round_trip_and_removal() {
    let id = uuid::Uuid::new_v4();
    let store = gamesync_desktop::credentials::SteamCredential::new(id).unwrap();
    assert!(store.get().unwrap().is_none());
    store.set("gamesync-disposable-test-value").unwrap();
    let fresh_entry = gamesync_desktop::credentials::SteamCredential::new(id).unwrap();
    let actual = fresh_entry.get().unwrap();
    let removed = store.remove();
    assert_eq!(actual.as_deref(), Some("gamesync-disposable-test-value"));
    removed.unwrap();
    assert!(store.get().unwrap().is_none());
}

#[test]
#[ignore = "Uses a dummy OS key and a separate process, never a Steam credential"]
fn native_key_survives_process_restart() {
    const PROBE: &str = "GAMESYNC_TEST_KEYCHAIN_ID";
    if let Ok(id) = std::env::var(PROBE) {
        let entry = gamesync_desktop::credentials::SteamCredential::new(
            uuid::Uuid::parse_str(&id).unwrap(),
        )
        .unwrap();
        assert_eq!(
            entry.get().unwrap().as_deref(),
            Some("disposable-process-test")
        );
        return;
    }
    let id = uuid::Uuid::new_v4();
    let entry = gamesync_desktop::credentials::SteamCredential::new(id).unwrap();
    entry.set("disposable-process-test").unwrap();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "native_key_survives_process_restart",
            "--ignored",
        ])
        .env(PROBE, id.to_string())
        .status();
    entry.remove().unwrap();
    assert!(result.unwrap().success());
}
