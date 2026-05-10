//! Domain-wall detection and sonification.
//!
//! A wall lives between sites i and i+1 where sign(sz[i]) != sign(sz[i+1]).
//! Walls are objects with persistent identity — born when a sign-flip appears,
//! destroyed when neighboring spins re-align. They drift, they get created in
//! pairs, they annihilate in pairs.
//!
//! This module owns the Wall and WallEvent types and (eventually) the
//! WallDetector that produces events from a chain. Voice allocation and MIDI
//! routing live in wall_midi.rs.

/// A domain wall as a tracked object.
#[derive(Clone, Debug)]
pub struct Wall {
    /// Persistent identity. Assigned at creation, not reused.
    pub id: u64,
    /// Sub-tick position. 2.5 means "between sites 2 and 3".
    pub position: f64,
    /// Position-per-tick. Computed from previous and current positions on
    /// each match. Zero on the tick of creation.
    pub velocity: f64,
    /// Tick when this wall was created.
    pub birth_tick: u64,
    /// Sign of the left side. +1 if sites left of the wall are positive,
    /// -1 if negative. Flips every drive period in the time-crystal phase.
    pub left_sign: i8,
}

/// An event emitted by the wall detector on a given tick.
#[derive(Clone, Debug)]
pub enum WallEvent {
    Created {
        id: u64,
        position: f64,
        tick: u64,
    },
    Destroyed {
        id: u64,
        last_position: f64,
        tick: u64,
        lifetime_ticks: u64,
    },
    Moved {
        id: u64,
        from: f64,
        to: f64,
        velocity: f64,
        tick: u64,
    },
}

use crate::chain::SpinChain;
use crate::config::WallConfig;

/// Watches a chain and produces walls each tick.
pub struct WallDetector {
    pub config: WallConfig,
    /// Walls present at the previous tick. Unused in Step 2; populated in Step 3.
    walls: Vec<Wall>,
    /// Monotonic counter for assigning new wall IDs. Unused in Step 2.
    next_id: u64,
}

impl WallDetector {
    pub fn new(config: WallConfig) -> Self {
        Self {
            config,
            walls: Vec::new(),
            next_id: 1,
        }
    }

    /// Scan the chain, match this tick's walls against the previous tick's,
    /// and emit Created / Destroyed / Moved events for the differences.
    pub fn check(&mut self, chain: &SpinChain) -> Vec<WallEvent> {
        if !self.config.enabled {
            return Vec::new();
        }

        // 1. Build candidate list — walls observed at this tick, no IDs yet.
        let mut candidates = self.scan_candidates(chain);

        // 2. Match candidates against previous walls. The matched_prev_idx
        //    array tracks which previous walls have been claimed.
        let n_prev = self.walls.len();
        let mut matched_prev = vec![false; n_prev];
        let mut candidate_match: Vec<Option<usize>> = vec![None; candidates.len()];

        // Compute all (candidate, previous, distance) triples within match_radius,
        // sort by distance, then greedily assign smallest-distance first.
        let mut pairs: Vec<(usize, usize, f64)> = Vec::new();
        for (ci, c) in candidates.iter().enumerate() {
            for (pi, p) in self.walls.iter().enumerate() {
                let dist = (c.position - p.position).abs();
                if dist <= self.config.match_radius {
                    pairs.push((ci, pi, dist));
                }
            }
        }
        pairs.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        for (ci, pi, _dist) in pairs {
            if candidate_match[ci].is_none() && !matched_prev[pi] {
                candidate_match[ci] = Some(pi);
                matched_prev[pi] = true;
            }
        }

        // 3. Emit events for matched, created, destroyed walls.
        let mut events: Vec<WallEvent> = Vec::new();

        // Created: candidates with no match. Assign fresh IDs.
        // Moved:   candidates with a match, position changed > move_threshold.
        // We also build the new wall list as we go.
        let mut new_walls: Vec<Wall> = Vec::with_capacity(candidates.len());

        for (ci, candidate) in candidates.iter_mut().enumerate() {
            match candidate_match[ci] {
                Some(pi) => {
                    let prev = &self.walls[pi];
                    candidate.id = prev.id;
                    candidate.birth_tick = prev.birth_tick;
                    candidate.velocity = candidate.position - prev.position;

                    let delta = (candidate.position - prev.position).abs();
                    if delta > self.config.move_threshold {
                        events.push(WallEvent::Moved {
                            id: candidate.id,
                            from: prev.position,
                            to: candidate.position,
                            velocity: candidate.velocity,
                            tick: chain.tick,
                        });
                    }
                    new_walls.push(candidate.clone());
                }
                None => {
                    candidate.id = self.next_id;
                    self.next_id += 1;
                    candidate.velocity = 0.0;
                    // birth_tick was already set to chain.tick in scan_candidates
                    events.push(WallEvent::Created {
                        id: candidate.id,
                        position: candidate.position,
                        tick: chain.tick,
                    });
                    new_walls.push(candidate.clone());
                }
            }
        }

        // Destroyed: previous walls with no match.
        for (pi, was_matched) in matched_prev.iter().enumerate() {
            if !was_matched {
                let prev = &self.walls[pi];
                events.push(WallEvent::Destroyed {
                    id: prev.id,
                    last_position: prev.position,
                    tick: chain.tick,
                    lifetime_ticks: chain.tick.saturating_sub(prev.birth_tick),
                });
            }
        }

        // 4. Replace the stored wall list with this tick's walls.
        self.walls = new_walls;

        events
    }

    /// Walk the chain and return one anonymous Wall per adjacent-site sign change.
    /// Pulled out as a helper so check() reads cleanly.
    fn scan_candidates(&self, chain: &SpinChain) -> Vec<Wall> {
        let mut candidates = Vec::new();
        let n = chain.spins.len();

        for i in 0..n.saturating_sub(1) {
            let left  = chain.sz(i);
            let right = chain.sz(i + 1);

            if left == 0.0 || right == 0.0 {
                continue;
            }

            if left.signum() != right.signum() {
                let position = if self.config.interpolate_position {
                    let l_abs = left.abs();
                    let r_abs = right.abs();
                    let denom = l_abs + r_abs;
                    // denom is the sum of two absolute values of nonzero numbers,
                    // so it's strictly positive. No divide-by-zero possible here.
                    i as f64 + l_abs / denom
                } else {
                    i as f64 + 0.5
                };

                candidates.push(Wall {
                    id: 0,
                    position,
                    velocity: 0.0,
                    birth_tick: chain.tick,
                    left_sign: if left > 0.0 { 1 } else { -1 },
                });
            }
        }

        candidates
    }
}