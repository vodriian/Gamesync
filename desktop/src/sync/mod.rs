//! Cross-device sync of personal data, settings, and keys. No UI or disk access.
//!
//! Each device writes its own changes. A change sets one field of one target
//! and lists the changes it replaced in `base`. The changes of all devices
//! form one history per field. One newest change is the field value. Several
//! unrelated newest changes are a conflict for the user to review. See
//! `plan/data-sync.md`.

mod change;
mod clock;
mod merge;

pub use change::{Change, Target};
pub use clock::{wall_ms, Clock, Stamp};
pub use merge::{ChangeSet, FieldKey, FieldState, Insert};
