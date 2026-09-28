//! Analytic critically damped springs. Retargeting retains both position and velocity.
use std::time::Instant;
const OMEGA: f32 = 26.;

pub struct Spring {
    position: f32,
    velocity: f32,
    target: f32,
    started: Instant,
}
impl Spring {
    pub fn new(value: f32) -> Self {
        Self {
            position: value,
            velocity: 0.,
            target: value,
            started: Instant::now(),
        }
    }
    fn sample(&self) -> (f32, f32) {
        response(
            self.position,
            self.velocity,
            self.target,
            self.started.elapsed().as_secs_f32(),
        )
    }
    pub fn value(&self) -> f32 {
        if self.active() {
            self.sample().0
        } else {
            self.target
        }
    }
    pub fn active(&self) -> bool {
        let (position, velocity) = self.sample();
        (position - self.target).abs() > 0.002 || velocity.abs() > 0.04
    }
    pub fn set(&mut self, target: f32) {
        if self.target == target {
            return;
        }
        let (position, velocity) = self.sample();
        self.position = position;
        self.velocity = velocity;
        self.target = target;
        self.started = Instant::now();
    }
}
fn response(position: f32, velocity: f32, target: f32, seconds: f32) -> (f32, f32) {
    let displacement = position - target;
    let coefficient = velocity + OMEGA * displacement;
    let decay = (-OMEGA * seconds).exp();
    (
        target + (displacement + coefficient * seconds) * decay,
        (velocity - OMEGA * coefficient * seconds) * decay,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_turn_settles_without_overshoot() {
        let target = std::f32::consts::PI;
        for i in 0..=450 {
            let (position, _) = response(0., 0., target, i as f32 / 1000.);
            assert!((0. ..=target).contains(&position));
        }
        let (position, velocity) = response(0., 0., target, 0.4);
        assert!((target - position).abs() < 0.002 && velocity.abs() < 0.04);
    }
    #[test]
    fn interruption_preserves_motion_then_reverses() {
        let (position, velocity) = response(0., 0., std::f32::consts::PI, 0.09);
        assert_eq!(response(position, velocity, 0., 0.), (position, velocity));
        assert!(response(position, velocity, 0., 0.12).1 < 0.);
        assert!(response(position, velocity, 0., 0.5).0.abs() < 0.002);
    }
}
