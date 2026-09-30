//! Hybrid logical clock. It orders changes across devices whose wall clocks
//! differ. It is not used to find conflicts; `base` does that.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// When a change was made. Ordered by time, then counter, then device, so two
/// stamps from different devices are never equal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct Stamp {
    /// Milliseconds since the Unix epoch, never lower than any stamp seen.
    pub ms: u64,
    /// Orders stamps within one millisecond.
    pub n: u32,
    pub device: Uuid,
}

/// One clock for each device. It never goes back, even when the wall clock does.
#[derive(Clone, Debug)]
pub struct Clock {
    device: Uuid,
    ms: u64,
    n: u32,
}

impl Clock {
    pub fn new(device: Uuid) -> Self {
        Self {
            device,
            ms: 0,
            n: 0,
        }
    }

    /// A stamp later than every stamp made or observed by this clock.
    pub fn tick(&mut self, wall_ms: u64) -> Stamp {
        if wall_ms > self.ms {
            self.ms = wall_ms;
            self.n = 0;
        } else if let Some(n) = self.n.checked_add(1) {
            self.n = n;
        } else {
            self.ms += 1;
            self.n = 0;
        }
        Stamp {
            ms: self.ms,
            n: self.n,
            device: self.device,
        }
    }

    /// Move past a received stamp, so a later local edit sorts after it even
    /// when the other device's clock is ahead.
    pub fn observe(&mut self, stamp: Stamp) {
        if (stamp.ms, stamp.n) > (self.ms, self.n) {
            self.ms = stamp.ms;
            self.n = stamp.n;
        }
    }
}

/// Current wall time in milliseconds. A clock before 1970 counts as 0.
pub fn wall_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| {
            u64::try_from(time.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_increase_when_the_wall_clock_stops_or_goes_back() {
        let mut clock = Clock::new(Uuid::from_u128(1));
        let first = clock.tick(1_000);
        let same = clock.tick(1_000);
        let back = clock.tick(500);
        assert!(first < same && same < back);
        assert_eq!(back.ms, 1_000);
        assert!(clock.tick(2_000) > back);
    }

    #[test]
    fn counter_overflow_moves_to_the_next_millisecond() {
        let mut clock = Clock::new(Uuid::from_u128(1));
        clock.observe(Stamp {
            ms: 10,
            n: u32::MAX,
            device: Uuid::from_u128(2),
        });
        let next = clock.tick(10);
        assert_eq!((next.ms, next.n), (11, 0));
    }

    #[test]
    fn a_slow_device_sorts_after_a_change_it_has_seen() {
        let mut fast = Clock::new(Uuid::from_u128(1));
        let mut slow = Clock::new(Uuid::from_u128(2));
        // The fast device is one hour ahead.
        let seen = fast.tick(3_600_000 + 5_000);
        slow.observe(seen);
        let edit = slow.tick(5_000);
        assert!(edit > seen);
    }
}
