//! A user-started session, independent of Steam process detection and playtime.
use super::{Choice, Context, LaunchOutcome};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivePlay {
    pub id: Uuid,
    pub library_id: Uuid,
    pub game_id: Uuid,
    /// Keep the session finishable if the game is hidden or removed.
    pub title: String,
    pub started_at: i64,
    pub context: Context,
}

impl ActivePlay {
    pub fn elapsed(&self, now: i64) -> u64 {
        now.saturating_sub(self.started_at).max(0) as u64
    }

    pub fn timer(&self, now: i64) -> String {
        let seconds = self.elapsed(now);
        if seconds >= 3600 {
            format!(
                "{}:{:02}:{:02}",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60
            )
        } else {
            format!("{:02}:{:02}", seconds / 60, seconds % 60)
        }
    }

    pub fn choice(&self, launch: LaunchOutcome, finished_at: Option<i64>) -> Choice {
        Choice {
            at: self.started_at,
            session: self.id,
            context: self.context.clone(),
            launch,
            finished_at: finished_at.map(|end| end.max(self.started_at)),
        }
    }
}
