use super::{brain_dead_ready, resolve, Context, EffectiveProfile, Effort, Scope, Stopping};
use crate::records::GameRevision;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub struct Candidate<'a> {
    pub record: &'a GameRevision,
    pub status_eligible: bool,
    /// None means this device has no verified installation information.
    pub installed: Option<bool>,
    /// Known blocker from the existing Best on assessment for the target.
    pub blocked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rejection {
    Hidden,
    Unavailable,
    Status,
    Excluded,
    Device,
    Scope,
    Installation,
    Activity,
    Time,
    Energy,
    BrainDead,
    Unknown,
}
impl Rejection {
    pub fn label(self) -> &'static str {
        match self {
            Self::Hidden => "Hidden",
            Self::Unavailable => "Unavailable",
            Self::Status => "Status",
            Self::Excluded => "Not interested",
            Self::Device => "Setup blocker",
            Self::Scope => "Library scope",
            Self::Installation => "Not installed on this setup",
            Self::Activity => "Activity",
            Self::Time => "Available time",
            Self::Energy => "Energy",
            Self::BrainDead => "Brain dead",
            Self::Unknown => "Incomplete profiles",
        }
    }
}

pub fn available(candidate: &Candidate<'_>) -> Result<(), Rejection> {
    let r = candidate.record;
    if r.game.personal.hidden {
        return Err(Rejection::Hidden);
    }
    if r.deleted || r.game.steam.as_ref().is_some_and(|s| !s.owned) {
        return Err(Rejection::Unavailable);
    }
    if !candidate.status_eligible {
        return Err(Rejection::Status);
    }
    if r.game.personal.play_now.excluded {
        return Err(Rejection::Excluded);
    }
    if candidate.blocked {
        return Err(Rejection::Device);
    }
    Ok(())
}

pub fn eligibility(
    candidate: &Candidate<'_>,
    context: &Context,
    profile: &EffectiveProfile,
) -> Result<(), Rejection> {
    available(candidate)?;
    let game = &candidate.record.game;
    if context.favorites_only && !game.personal.favorite {
        return Err(Rejection::Scope);
    }
    if context.scope == Scope::Unplayed
        && game.steam.as_ref().is_none_or(|s| s.playtime_minutes != 0)
    {
        return Err(Rejection::Scope);
    }
    if context.scope == Scope::Playing && game.personal.status != "playing" {
        return Err(Rejection::Scope);
    }
    if context.installed_only && candidate.installed != Some(true) {
        return Err(Rejection::Installation);
    }
    let p = &profile.values;
    if let Some(activity) = context.activity {
        if !p
            .activities
            .as_ref()
            .ok_or(Rejection::Unknown)?
            .contains(&activity)
        {
            return Err(Rejection::Activity);
        }
    }
    // Known failures win over incomplete evidence in other fields.
    if p.mechanical.is_some_and(|v| v > context.energy)
        || p.cognitive.is_some_and(|v| v > context.energy)
    {
        return Err(Rejection::Energy);
    }
    if let Some(minutes) = context.minutes {
        if p.minimum_minutes.is_some_and(|m| m > minutes) {
            return Err(Rejection::Time);
        }
        let (minimum, setup) = (
            p.minimum_minutes.ok_or(Rejection::Unknown)?,
            p.setup_minutes.ok_or(Rejection::Unknown)?,
        );
        if u32::from(minimum) + u32::from(setup) > u32::from(minutes) {
            return Err(Rejection::Time);
        }
    }
    p.energy().ok_or(Rejection::Unknown)?;
    if context.brain_dead && !brain_dead_ready(p) {
        if p.cognitive.is_some_and(|v| v > Effort::Low)
            || p.narrative.is_some_and(|v| v > Effort::Low)
            || p.onboarding.is_some_and(|v| v > Effort::Low)
            || p.stopping.is_some_and(|v| v != Stopping::Flexible)
            || p.setup_minutes.is_some_and(|v| v > 3)
        {
            return Err(Rejection::BrainDead);
        }
        if p.narrative.is_none()
            || p.onboarding.is_none()
            || p.stopping.is_none()
            || p.setup_minutes.is_none()
        {
            return Err(Rejection::Unknown);
        }
        return Err(Rejection::BrainDead);
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Pick {
    pub id: Uuid,
    pub profile: EffectiveProfile,
    pub score: i32,
    pub reason: String,
}
#[derive(Default)]
pub struct Selection {
    pub ranked: Vec<Pick>,
    pub rejected: BTreeMap<Rejection, usize>,
    pub incomplete: Vec<Uuid>,
}

/// `now` is supplied by the caller. Stable IDs break ties, including tied titles.
pub fn rank(candidates: &[Candidate<'_>], context: &Context, now: i64) -> Selection {
    let mut out = Selection::default();
    let mut unique = BTreeSet::new();
    for candidate in candidates {
        let record = candidate.record;
        if !unique.insert(record.game_id) {
            continue;
        }
        let profile = resolve(&record.game);
        if let Err(reason) = eligibility(candidate, context, &profile) {
            *out.rejected.entry(reason).or_default() += 1;
            if reason == Rejection::Unknown {
                out.incomplete.push(record.game_id);
            }
            continue;
        }
        let game = &record.game;
        let mut score = 100;
        if let (Some(available), Some(ideal)) = (context.minutes, profile.values.ideal_minutes) {
            score -= i32::from(available.abs_diff(ideal).min(60)) / 3;
        }
        score -= i32::from(profile.values.setup_minutes.unwrap_or(5));
        score += if profile.values.stopping == Some(Stopping::Flexible) {
            4
        } else {
            0
        };
        score += if game.personal.favorite { 4 } else { 0 };
        score += i32::from(game.personal.rating.unwrap_or(0)) / 2;
        score += if game.personal.status == "playing" {
            3
        } else {
            0
        };
        score -= if profile.estimated() { 5 } else { 0 };
        let rediscovered = game
            .steam
            .as_ref()
            .and_then(|s| s.last_played)
            .is_some_and(|last| last > 0 && now.saturating_sub(last) >= 90 * 86400);
        if rediscovered {
            score += 2;
        }
        let reason = if context.brain_dead {
            "Little thinking or story recall, with flexible stopping.".to_owned()
        } else if let Some(activity) = context.activity {
            format!(
                "Fits {} and your {} energy budget.",
                activity.label().to_lowercase(),
                context.energy.label().to_lowercase()
            )
        } else if rediscovered {
            "A game you have not played for a while, within these limits.".into()
        } else if game.personal.status == "playing" {
            "Continue a familiar game that fits these limits.".into()
        } else {
            "Fits your available time and energy.".into()
        };
        out.ranked.push(Pick {
            id: record.game_id,
            profile,
            score,
            reason,
        });
    }
    out.ranked
        .sort_by(|a, b| b.score.cmp(&a.score).then(a.id.cmp(&b.id)));
    out
}

pub struct Session {
    pub id: Uuid,
    pub context: Context,
    pub hand: Vec<Uuid>,
    pub shown: BTreeSet<Uuid>,
    pub dismissed: BTreeSet<Uuid>,
    pub reshuffles: u8,
    dealt: bool,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            context: Context::default(),
            hand: Vec::new(),
            shown: BTreeSet::new(),
            dismissed: BTreeSet::new(),
            reshuffles: 1,
            dealt: false,
        }
    }
}
impl Session {
    /// Editing unchanged preferences keeps the same hand, including dismissals.
    /// Revalidation removes games that became hidden or otherwise ineligible.
    pub fn deal(&mut self, context: Context, ranked: &[Pick], reshuffle: bool) {
        let valid: BTreeSet<_> = ranked.iter().map(|p| p.id).collect();
        if reshuffle
            && (self.reshuffles == 0
                || !ranked
                    .iter()
                    .any(|p| !self.shown.contains(&p.id) && !self.dismissed.contains(&p.id)))
        {
            return;
        }
        if !reshuffle && self.dealt && self.context == context {
            self.hand.retain(|id| valid.contains(id));
            return;
        }
        if reshuffle {
            self.reshuffles -= 1;
        }
        self.context = context;
        let mut pool: Vec<_> = ranked
            .iter()
            .filter(|p| {
                !self.dismissed.contains(&p.id) && (!reshuffle || !self.shown.contains(&p.id))
            })
            .collect();
        self.hand.clear();
        let mut used_activities = BTreeSet::new();
        while !pool.is_empty() && self.hand.len() < 3 {
            let best_score = pool[0].score;
            let index = if self.hand.is_empty() {
                0
            } else {
                pool.iter()
                    .position(|p| {
                        p.score >= best_score - 5
                            && p.profile
                                .values
                                .activities
                                .as_ref()
                                .is_some_and(|a| a.iter().any(|a| !used_activities.contains(a)))
                    })
                    .unwrap_or(0)
            };
            let pick = pool.remove(index);
            used_activities.extend(pick.profile.values.activities.iter().flatten().copied());
            self.hand.push(pick.id);
            self.shown.insert(pick.id);
        }
        self.dealt = true;
    }
}
