//! The run's steady flow, solved once at load (T38): the standard step
//! method from the energy equation, in both regimes, as a river engineer's
//! backwater program does it (HEC-RAS's mixed-flow mode, the textbook way).
//!
//! The channel is cut into one-metre steps, each a wide rectangle with a bed
//! height, a width and a roughness (Manning's n). The discharge is shared by
//! every step, so each has its own flow per metre of width q = Q / w.
//!
//! 1. Calm (subcritical) water is controlled from downstream: from the exit
//!    lip, where the water goes critical, the total head H = z + h + v²/2g is
//!    carried upstream a step at a time, gaining the friction lost over the
//!    step. Where the head left over the bed is less than the critical
//!    energy 1.5 h_c, no calm water can pass: the water goes critical there.
//!    That is a lip, and the top of every rapid finds its own.
//! 2. Rushing (supercritical) water is controlled from upstream: from each
//!    lip it is carried downstream the same way, losing the friction.
//! 3. Where both could be, the one with the greater specific force
//!    M = q²/(g h) + h²/2 wins; where the rushing water's force first falls
//!    below the calm water's, it jumps (a hydraulic jump), and the depths
//!    either side meet Bélanger's h₂ = h₁/2 (√(1 + 8 Fr₁²) − 1).
//!
//! Friction: S_f = n² q² / h^(10/3), Manning's formula for a wide channel.
//! The sums run in Q32 (64-bit values with 32 fraction bits): a rapid's
//! depth to the 10/3 power is too small for Q16.

/// A Q32 value: 32 fraction bits.
pub type F = i64;
pub const ONE: F = 1 << 32;
/// Gravity, 9.81 m/s².
pub const G: F = 981 * ONE / 100;

pub fn mul(a: F, b: F) -> F {
    ((a as i128 * b as i128) >> 32) as F
}

pub fn div(a: F, b: F) -> F {
    (((a as i128) << 32) / (b as i128).max(1)) as F
}

fn isqrt(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = 1u128 << (128 - n.leading_zeros()).div_ceil(2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

pub fn sqrt(x: F) -> F {
    if x <= 0 {
        return 0;
    }
    isqrt((x as u128) << 32) as F
}

/// The cube root of a non-negative value, by halving.
pub fn cbrt(x: F) -> F {
    if x <= 0 {
        return 0;
    }
    let (mut lo, mut hi) = (0, x.max(ONE));
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if mul(mul(mid, mid), mid) <= x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Q16 to Q32 and back.
pub fn from16(v: i32) -> F {
    (v as F) << 16
}
pub fn to16(v: F) -> i32 {
    (v >> 16).clamp(i32::MIN as F, i32::MAX as F) as i32
}

/// One step of the channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cross {
    /// The bed's mean height, metres.
    pub bed: F,
    pub width: F,
    /// Manning's n.
    pub n: F,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Regime {
    /// Calm: slower than a wave can run (Fr < 1).
    Calm,
    /// Rushing: faster than a wave can run upstream (Fr > 1).
    Rushing,
    /// At a lip: Fr = 1.
    Critical,
}

/// The solved flow at one step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flow {
    pub depth: F,
    pub speed: F,
    pub froude: F,
    /// The total head, metres: bed, depth and velocity head.
    pub head: F,
    pub regime: Regime,
}

/// A hydraulic jump: rushing water at depth `h1` and Froude number `fr1`
/// jumps to calm water at depth `h2`, at step `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Jump {
    pub at: usize,
    pub h1: F,
    pub h2: F,
    pub v1: F,
    pub fr1: F,
}

struct Step {
    q: F,
    q2: F,
    hc: F,
}

impl Step {
    fn new(c: &Cross, discharge: F) -> Step {
        let q = div(discharge, c.width.max(ONE));
        let q2 = mul(q, q);
        Step {
            q,
            q2,
            hc: cbrt(div(q2, G)),
        }
    }

    /// Specific energy: h + q²/(2 g h²).
    fn energy(&self, h: F) -> F {
        h + div(self.q2, mul(2 * G, mul(h, h)).max(1))
    }

    /// Specific force per metre of width: q²/(g h) + h²/2.
    fn force(&self, h: F) -> F {
        div(self.q2, mul(G, h).max(1)) + mul(h, h) / 2
    }

    /// The friction slope at depth h: n² q² / h^(10/3).
    fn friction(&self, n: F, h: F) -> F {
        let h103 = mul(mul(mul(h, h), h), cbrt(h)).max(1);
        div(mul(mul(n, n), self.q2), h103)
    }

    /// The depth with specific energy `e` on the calm side (above h_c) or
    /// the rushing side (below it); None when `e` is under the critical.
    fn depth(&self, e: F, calm: bool) -> Option<F> {
        if e < self.energy(self.hc) {
            return None;
        }
        let (mut lo, mut hi) = if calm {
            (self.hc, e.max(self.hc))
        } else {
            (ONE / 1000, self.hc)
        };
        for _ in 0..48 {
            let mid = lo + (hi - lo) / 2;
            let too_much = self.energy(mid) > e;
            // energy rises with depth on the calm side and falls on the rushing side
            if too_much == calm {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        Some(lo + (hi - lo) / 2)
    }

    fn flow(&self, bed: F, h: F, regime: Regime) -> Flow {
        let speed = div(self.q, h.max(1));
        Flow {
            depth: h,
            speed,
            froude: div(speed, sqrt(mul(G, h))),
            head: bed + self.energy(h),
            regime,
        }
    }
}

/// Solves the steady flow of `discharge` (m³/s) down the steps, each `dx`
/// metres long, the water leaving over the last step at critical depth.
pub fn solve(steps: &[Cross], discharge: F, dx: F) -> (Vec<Flow>, Vec<Jump>) {
    let n = steps.len();
    let st: Vec<Step> = steps.iter().map(|c| Step::new(c, discharge)).collect();
    // 1. the calm water, from the exit up
    let mut calm: Vec<Option<F>> = vec![None; n];
    let mut head = vec![0; n];
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    head[n - 1] = steps[n - 1].bed + st[n - 1].energy(st[n - 1].hc);
    let mut below = st[n - 1].hc;
    for i in (0..n - 1).rev() {
        let lose = mul(st[i + 1].friction(steps[i + 1].n, below), dx);
        // a predictor with the friction below, a corrector with the mean
        let mut h = st[i].depth(head[i + 1] + lose - steps[i].bed, true);
        if let Some(h0) = h {
            let mean = (st[i].friction(steps[i].n, h0) + st[i + 1].friction(steps[i + 1].n, below)) / 2;
            h = st[i].depth(head[i + 1] + mul(mean, dx) - steps[i].bed, true);
        }
        calm[i] = h;
        match h {
            Some(h) => {
                head[i] = steps[i].bed + st[i].energy(h);
                below = h;
            }
            None => {
                // no calm water passes here: a lip
                head[i] = steps[i].bed + st[i].energy(st[i].hc);
                below = st[i].hc;
            }
        }
    }
    // 2 and 3. the rushing water from each lip down, and the jumps
    let mut out = Vec::with_capacity(n);
    let mut jumps = Vec::new();
    let mut rushing: Option<(F, F)> = None; // (head, depth) of the rushing water above
    for i in 0..n {
        let s = &st[i];
        let bed = steps[i].bed;
        let sup = rushing.and_then(|(h_above, d_above)| {
            let lose = mul(st[i - 1].friction(steps[i - 1].n, d_above), dx);
            let h = s.depth(h_above - lose - bed, false)?;
            let mean = (s.friction(steps[i].n, h) + st[i - 1].friction(steps[i - 1].n, d_above)) / 2;
            s.depth(h_above - mul(mean, dx) - bed, false)
        });
        let flow = match (sup, calm[i]) {
            (Some(h1), Some(h2)) if s.force(h1) > s.force(h2) => s.flow(bed, h1, Regime::Rushing),
            (Some(h1), Some(h2)) => {
                let f = s.flow(bed, h1, Regime::Rushing);
                jumps.push(Jump {
                    at: i,
                    h1,
                    h2,
                    v1: f.speed,
                    fr1: f.froude,
                });
                s.flow(bed, h2, Regime::Calm)
            }
            (None, Some(h)) => s.flow(bed, h, Regime::Calm),
            (Some(h), None) => s.flow(bed, h, Regime::Rushing),
            // a lip: the water goes critical and rushes on from here
            (None, None) => s.flow(bed, s.hc, Regime::Critical),
        };
        rushing = match flow.regime {
            Regime::Calm => None,
            _ => Some((flow.head, flow.depth)),
        };
        out.push(flow);
    }
    (out, jumps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(v: i64, per: i64) -> F {
        v * ONE / per
    }

    fn near(a: F, b: F, tol: F) -> bool {
        (a - b).abs() <= tol
    }

    /// `len` metres falling `slope` (per thousand), 30 m wide, n per thousand.
    fn reach(top: F, len: usize, slope: i64, n: i64) -> Vec<Cross> {
        (0..len)
            .map(|i| Cross {
                bed: top - m(slope * i as i64, 1000),
                width: m(30, 1),
                n: m(n, 1000),
            })
            .collect()
    }

    /// 90 m³/s over 30 m: q = 3 m²/s.
    const Q: i64 = 90;

    #[test]
    fn critical_depth_and_the_roots_of_the_energy_are_worked_by_hand() {
        let s = Step::new(
            &Cross {
                bed: 0,
                width: m(30, 1),
                n: 0,
            },
            m(Q, 1),
        );
        // h_c = ∛(q²/g) = ∛(9 / 9.81) = 0.97165 m
        assert!(
            near(s.hc, m(97165, 100_000), m(1, 10_000)),
            "{}",
            s.hc * 1000 / ONE
        );
        // E = 2 m: calm root 1.8686 (1.8686 + 9/(19.62 × 3.4917) = 2.0000),
        // rushing root 0.5655 (0.5655 + 9/(19.62 × 0.31979) = 2.0000)
        let calm = s.depth(m(2, 1), true).unwrap();
        let rush = s.depth(m(2, 1), false).unwrap();
        assert!(
            near(calm, m(18686, 10_000), m(5, 10_000)),
            "{}",
            calm * 1000 / ONE
        );
        assert!(near(rush, m(5655, 10_000), m(5, 10_000)), "{}", rush * 1000 / ONE);
        // under 1.5 h_c = 1.4575 m there is no root at all
        assert_eq!(s.depth(m(14, 10), true), None);
    }

    #[test]
    fn a_long_steep_reach_runs_at_its_normal_depth_and_rushes() {
        // a 400 m reach at 5%, n = 0.05: (q n / √S)^(3/5) = (0.15 / 0.22361)^0.6 = 0.7870 m,
        // v = 3/0.787 = 3.812 m/s, Fr = 3.812 / √(9.81 × 0.787) = 1.372
        let (f, _) = solve(&reach(m(20, 1), 400, 50, 50), m(Q, 1), ONE);
        // the top is a lip: critical depth
        assert_eq!(f[0].regime, Regime::Critical);
        let mid = f[300];
        assert_eq!(mid.regime, Regime::Rushing);
        assert!(
            near(mid.depth, m(7870, 10_000), m(5, 1000)),
            "{}",
            mid.depth * 1000 / ONE
        );
        assert!(
            near(mid.froude, m(1372, 1000), m(2, 100)),
            "{}",
            mid.froude * 1000 / ONE
        );
    }

    #[test]
    fn a_long_mild_reach_runs_calm_at_its_normal_depth() {
        // 6 km at 0.1%, n = 0.035: (0.105 / 0.031623)^0.6 = 2.0545 m. A mild
        // reach's backwater from its exit runs kilometres (h/S is 2 km), so
        // the normal depth is looked for 5.9 km up
        let (f, _) = solve(&reach(m(10, 1), 6000, 1, 35), m(Q, 1), ONE);
        assert_eq!(f[100].regime, Regime::Calm);
        assert!(
            near(f[100].depth, m(20545, 10_000), m(1, 100)),
            "{}",
            f[100].depth * 1000 / ONE
        );
        // the exit is critical
        assert_eq!(f[5999].regime, Regime::Critical);
    }

    #[test]
    fn a_pool_stands_one_and_a_half_critical_depths_over_its_lip() {
        // a level, deep pool (bed 3 m down) runs to a lip at 0 m, then a 5% rapid
        let mut steps: Vec<Cross> = (0..100)
            .map(|_| Cross {
                bed: -m(3, 1),
                width: m(30, 1),
                n: m(35, 1000),
            })
            .collect();
        steps.extend(reach(0, 200, 50, 50));
        let (f, _) = solve(&steps, m(Q, 1), ONE);
        // the lip is the rapid's first step; the pool's surface over it is
        // 0 + 1.5 × 0.97165 = 1.4575 m at the lip; the step up from it gains
        // half the lip's friction (0.0225 / 0.97^(10/3) / 2 = 0.0125 m) and
        // the pool's own over 49 m (0.004 m), and the surface stands the
        // pool's velocity head below its head (4.45 m deep: 0.674 m/s,
        // 0.023 m): 1.4575 + 0.0125 + 0.004 − 0.023 = 1.451 m
        let lip = f.iter().position(|x| x.regime == Regime::Critical).unwrap();
        assert_eq!(lip, 100);
        let surface = steps[50].bed + f[50].depth;
        assert!(
            near(surface, m(1451, 1000), m(5, 1000)),
            "{}",
            surface * 1000 / ONE
        );
        assert_eq!(f[150].regime, Regime::Rushing);
    }

    #[test]
    fn rushing_water_meeting_a_deep_tail_jumps_to_belangers_depth() {
        // 300 m at 5% then 600 m at 0.1%, n = 0.05 throughout: the rushing
        // water (0.787 m, Fr 1.372) meets a tail 2.54 m deep, deeper than its
        // sequent 0.787/2 (√(1 + 8 × 1.372²) − 1) = 1.183 m, so the jump climbs
        // the slope to where the tail is that deep: the tail's energy 2.616 m
        // (2.545 + 0.071) less that at 1.183 m (1.511 m), 1.105 m, plus about
        // 0.11 m of friction, is 1.2 m of bed: some 24 m above the break
        let mut steps = reach(m(20, 1), 300, 50, 50);
        let top = steps[299].bed - m(50, 1000);
        steps.extend(reach(top, 600, 1, 50));
        let (f, jumps) = solve(&steps, m(Q, 1), ONE);
        assert_eq!(jumps.len(), 1, "{jumps:?}");
        let j = jumps[0];
        assert!((276..=282).contains(&j.at), "the jump at {}", j.at);
        // Bélanger, from the depths the solver found either side
        let fr2 = mul(j.fr1, j.fr1);
        let want = mul(j.h1 / 2, sqrt(ONE + 8 * fr2) - ONE);
        assert!(
            near(j.h2, want, m(3, 100)),
            "h2 {} want {}",
            j.h2 * 1000 / ONE,
            want * 1000 / ONE
        );
        assert_eq!(f[j.at - 1].regime, Regime::Rushing);
        assert_eq!(f[j.at].regime, Regime::Calm);
    }
}
