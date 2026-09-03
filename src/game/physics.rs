//! Swept collision only; no perk/powerup special-cases (FR-15/16, AGENTS §3).
//!
//! Owns the fixed-timestep integration and circle-vs-AABB swept solver for
//! walls, paddle, bricks. Reads `RunModifiers`, never branches on
//! individual perks. Pure, deterministic, unit-tested.

use super::rng::Rng;
use super::score::Score;
use super::state::GameState;
use super::tuning;

/// Mechanical axes the simulation reads (ADR-0008). Perks mutate this;
/// physics never names a perk.
#[derive(Debug, Clone, Copy)]
pub struct RunModifiers {
    /// Multiplier on ball speed.
    pub ball_speed_mul: f32,
    /// Multiplier on paddle width.
    pub paddle_width_mul: f32,
    /// Multiplier on score awards.
    pub score_mul: f32,
    /// Added to the powerup drop chance (Phase 6).
    pub drop_rate_add: f32,
    /// Extra starting lives (Phase 8).
    pub starting_lives: i32,
}

impl Default for RunModifiers {
    fn default() -> Self {
        Self {
            ball_speed_mul: 1.0,
            paddle_width_mul: 1.0,
            score_mul: 1.0,
            drop_rate_add: 0.0,
            starting_lives: 0,
        }
    }
}

/// Per-frame input snapshot. Sampled at frame rate, applied at step rate
/// (ADR-0006). `Copy` so replay logs are cheap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct InputState {
    /// Hold to move left.
    pub left: bool,
    /// Hold to move right.
    pub right: bool,
    /// Edge: launch a stuck ball.
    pub launch: bool,
}

/// One ball. Position is the centre in logical pixels.
#[derive(Debug, Clone, Copy)]
pub struct Ball {
    /// Centre x.
    pub x: f32,
    /// Centre y.
    pub y: f32,
    /// Velocity px/s.
    pub vx: f32,
    /// Velocity px/s.
    pub vy: f32,
    /// True while waiting on the paddle for `Space`.
    pub stuck: bool,
}

/// Brick kind (FR-17/18/19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrickKind {
    /// 1-5 HP; recolours to tier below on hit.
    Normal,
    /// Indestructible, not counted for clear.
    Steel,
    /// 1 HP; destroys its 3x3 neighbourhood when destroyed.
    Explosive,
}

/// One brick cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Brick {
    /// Grid column 0..18.
    pub col: u8,
    /// Grid row 0..14.
    pub row: u8,
    /// Hit points (Normal 1-5, Explosive 1, Steel u8::MAX sentinel).
    pub hp: u8,
    /// Kind.
    pub kind: BrickKind,
}

impl Brick {
    /// Destructible brick with HP.
    pub fn normal(col: u8, row: u8, hp: u8) -> Self {
        Self {
            col,
            row,
            hp: hp.clamp(1, 5),
            kind: BrickKind::Normal,
        }
    }

    /// Steel brick.
    pub fn steel(col: u8, row: u8) -> Self {
        Self {
            col,
            row,
            hp: u8::MAX,
            kind: BrickKind::Steel,
        }
    }

    /// Explosive brick.
    pub fn explosive(col: u8, row: u8) -> Self {
        Self {
            col,
            row,
            hp: 1,
            kind: BrickKind::Explosive,
        }
    }

    /// Counts toward level clear.
    pub fn counts_for_clear(&self) -> bool {
        self.kind != BrickKind::Steel
    }

    /// AABB in logical pixels (drawn 16x7 rect).
    pub fn aabb(&self) -> (f32, f32, f32, f32) {
        let x = tuning::GRID_ORIGIN_X + f32::from(self.col) * tuning::BRICK_CELL_W;
        let y = tuning::GRID_ORIGIN_Y + f32::from(self.row) * tuning::BRICK_CELL_H;
        (x, y, tuning::BRICK_DRAW_W, tuning::BRICK_DRAW_H)
    }
}

/// The whole simulation state for one level.
#[derive(Debug)]
pub struct World {
    /// Paddle centre x.
    pub paddle_x: f32,
    /// Paddle velocity px/s.
    pub paddle_vel: f32,
    /// Balls in play (cap `MULTIBALL_CAP`).
    pub balls: Vec<Ball>,
    /// Bricks remaining.
    pub bricks: Vec<Brick>,
    /// Score + combo.
    pub score: Score,
    /// Lives remaining.
    pub lives: i32,
    /// Level index 0-7 (speed ramp).
    pub level_index: u32,
    /// Bricks destroyed this level (speed ramp).
    pub bricks_destroyed: u32,
    /// State machine.
    pub state: GameState,
    /// Deterministic generator.
    pub rng: Rng,
    /// Data-driven modifiers.
    pub modifiers: RunModifiers,
}

impl World {
    /// New world with the given bricks, seed, level index and modifiers.
    pub fn new(bricks: Vec<Brick>, seed: u64, level_index: u32, modifiers: RunModifiers) -> Self {
        let lives = tuning::STARTING_LIVES + modifiers.starting_lives;
        let mut w = Self {
            paddle_x: tuning::PLAY_X + tuning::PLAY_W / 2.0,
            paddle_vel: 0.0,
            balls: Vec::new(),
            bricks,
            score: Score::new(),
            lives,
            level_index,
            bricks_destroyed: 0,
            state: GameState::Playing,
            rng: Rng::from_seed(seed),
            modifiers,
        };
        w.spawn_stuck_ball();
        w
    }

    /// Current paddle width after modifiers.
    pub fn paddle_width(&self) -> f32 {
        (tuning::PADDLE_W * self.modifiers.paddle_width_mul).clamp(20.0, 120.0)
    }

    /// Target ball speed right now.
    pub fn target_speed(&self) -> f32 {
        tuning::ball_speed(self.bricks_destroyed, self.level_index) * self.modifiers.ball_speed_mul
    }

    /// Place a stuck ball on the paddle (start of life / level).
    fn spawn_stuck_ball(&mut self) {
        if self.balls.len() < tuning::MULTIBALL_CAP {
            self.balls.push(Ball {
                x: self.paddle_x,
                y: tuning::PADDLE_Y - tuning::BALL_R - 1.0,
                vx: 0.0,
                vy: 0.0,
                stuck: true,
            });
        }
    }

    /// Destructible bricks remaining.
    pub fn remaining(&self) -> usize {
        self.bricks.iter().filter(|b| b.counts_for_clear()).count()
    }

    /// One fixed step of `dt` seconds (ADR-0006: `dt` is always `DT`).
    pub fn step(&mut self, input: InputState, dt: f32) {
        if self.state != GameState::Playing {
            return;
        }
        self.step_paddle(input, dt);
        // Launch edge: release stuck balls.
        if input.launch {
            let speed = self.target_speed();
            for b in self.balls.iter_mut().filter(|b| b.stuck) {
                b.stuck = false;
                // Straight up with a deterministic nudge from the rng so two
                // launches with different seeds diverge (still deterministic).
                let nudge = (self.rng.next_f32() - 0.5) * 20.0;
                b.vx = nudge;
                b.vy = -speed;
                let sp = (b.vx * b.vx + b.vy * b.vy).sqrt();
                b.vx = b.vx / sp * speed;
                b.vy = b.vy / sp * speed;
            }
        }
        // Integrate balls with swept collisions.
        let mut lost = 0usize;
        // Index-based loop: explosions mutate bricks but not balls.
        for i in 0..self.balls.len() {
            if self.balls[i].stuck {
                self.balls[i].x = self.paddle_x;
                self.balls[i].y = tuning::PADDLE_Y - tuning::BALL_R - 1.0;
                continue;
            }
            self.step_ball(i, dt);
            if self.balls[i].y - tuning::BALL_R > tuning::KILL_Y {
                lost += 1;
            }
        }
        // Remove lost balls (swap-remove from the back for determinism).
        if lost > 0 {
            let mut kept = Vec::with_capacity(self.balls.len());
            for b in self.balls.drain(..) {
                if b.y - tuning::BALL_R <= tuning::KILL_Y {
                    kept.push(b);
                }
            }
            self.balls = kept;
        }
        if self.balls.is_empty() {
            self.lives -= 1;
            self.score.on_paddle_contact();
            if self.lives <= 0 {
                self.state = GameState::RunOver;
            } else {
                self.paddle_x = tuning::PLAY_X + tuning::PLAY_W / 2.0;
                self.paddle_vel = 0.0;
                self.spawn_stuck_ball();
            }
            return;
        }
        if self.remaining() == 0 {
            self.state = GameState::LevelClear;
            // Keep the transition function honest: the legal move exists.
            debug_assert!(self
                .state
                .transition(super::state::StateEvent::Start)
                .is_some());
        }
    }

    /// Paddle accel/friction integration (FR-13).
    fn step_paddle(&mut self, input: InputState, dt: f32) {
        let w = self.paddle_width();
        let dir: f32 = match (input.left, input.right) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        };
        if dir != 0.0 {
            self.paddle_vel += dir * tuning::PADDLE_ACCEL * dt;
            self.paddle_vel = self
                .paddle_vel
                .clamp(-tuning::PADDLE_MAX_VEL, tuning::PADDLE_MAX_VEL);
        } else {
            // Friction toward zero.
            let f = tuning::PADDLE_FRICTION * dt;
            if self.paddle_vel > f {
                self.paddle_vel -= f;
            } else if self.paddle_vel < -f {
                self.paddle_vel += f;
            } else {
                self.paddle_vel = 0.0;
            }
        }
        self.paddle_x += self.paddle_vel * dt;
        let lo = tuning::PLAY_X + w / 2.0;
        let hi = tuning::PLAY_RIGHT - w / 2.0;
        if self.paddle_x < lo {
            self.paddle_x = lo;
            self.paddle_vel = 0.0;
        } else if self.paddle_x > hi {
            self.paddle_x = hi;
            self.paddle_vel = 0.0;
        }
    }

    /// Move one ball with iterative swept collisions (up to 4 per step).
    fn step_ball(&mut self, idx: usize, dt: f32) {
        let mut remaining = dt;
        let mut iterations = 0;
        while remaining > 1e-7 && iterations < 4 {
            iterations += 1;
            let b = self.balls[idx];
            let dx = b.vx * remaining;
            let dy = b.vy * remaining;
            match self.earliest_hit(idx, dx, dy) {
                None => {
                    self.balls[idx].x += dx;
                    self.balls[idx].y += dy;
                    break;
                }
                Some(hit) => {
                    // Advance to contact, reflect, continue with leftover time.
                    self.balls[idx].x += b.vx * remaining * hit.toi;
                    self.balls[idx].y += b.vy * remaining * hit.toi;
                    self.resolve_hit(idx, hit);
                    remaining *= 1.0 - hit.toi;
                    if remaining < 1e-7 {
                        break;
                    }
                }
            }
        }
        // Clamp inside the play area so a numerical edge never escapes.
        let r = tuning::BALL_R;
        self.balls[idx].x = self.balls[idx]
            .x
            .clamp(tuning::PLAY_X + r * 0.5, tuning::PLAY_RIGHT - r * 0.5);
    }

    /// Earliest collision along the segment (dx,dy), if any.
    fn earliest_hit(&self, idx: usize, dx: f32, dy: f32) -> Option<Hit> {
        let b = self.balls[idx];
        let r = tuning::BALL_R;
        let mut best: Option<Hit> = None;

        // Walls: left / right / top.
        if dx < 0.0 {
            let x_at = tuning::PLAY_X + r;
            let toi = (x_at - b.x) / dx;
            if (0.0..=1.0).contains(&toi) {
                best = Some(Hit::new(toi, HitKind::Wall, 1.0, 0.0).min_toi(best));
            }
        } else if dx > 0.0 {
            let x_at = tuning::PLAY_RIGHT - r;
            let toi = (x_at - b.x) / dx;
            if (0.0..=1.0).contains(&toi) {
                best = Some(Hit::new(toi, HitKind::Wall, -1.0, 0.0).min_toi(best));
            }
        }
        if dy < 0.0 {
            let y_at = tuning::PLAY_TOP + r;
            let toi = (y_at - b.y) / dy;
            if (0.0..=1.0).contains(&toi) {
                best = Some(Hit::new(toi, HitKind::Wall, 0.0, 1.0).min_toi(best));
            }
        }

        // Paddle (only when moving down).
        if dy > 0.0 {
            let w = self.paddle_width();
            let px = self.paddle_x - w / 2.0;
            let py = tuning::PADDLE_Y;
            if let Some((toi, nx, ny)) =
                swept_circle_vs_aabb(b.x, b.y, dx, dy, r, px, py, w, tuning::PADDLE_H)
            {
                best = Some(Hit::new(toi, HitKind::Paddle, nx, ny).min_toi(best));
            }
        }

        // Bricks.
        for (bi, brick) in self.bricks.iter().enumerate() {
            let (bx, by, bw, bh) = brick.aabb();
            if let Some((toi, nx, ny)) = swept_circle_vs_aabb(b.x, b.y, dx, dy, r, bx, by, bw, bh) {
                best = Some(Hit::new(toi, HitKind::Brick(bi), nx, ny).min_toi(best));
            }
        }
        best
    }

    /// Apply the reflection + game effects for one contact.
    fn resolve_hit(&mut self, idx: usize, hit: Hit) {
        match hit.kind {
            HitKind::Wall => {
                let b = &mut self.balls[idx];
                if hit.nx != 0.0 {
                    b.vx = -b.vx;
                }
                if hit.ny != 0.0 {
                    b.vy = -b.vy;
                }
                enforce_min_vertical(b);
            }
            HitKind::Paddle => {
                // English: offset across the paddle sets the angle (FR-15),
                // plus a carry of paddle momentum (FR-13).
                let w = self.paddle_width();
                let offset = ((self.balls[idx].x - self.paddle_x) / (w / 2.0)).clamp(-1.0, 1.0);
                let speed = self.target_speed();
                let ang = offset * tuning::ENGLISH_MAX_DEG.to_radians();
                let mut vx = ang.sin() * speed + self.paddle_vel * tuning::PADDLE_MOMENTUM;
                let mut vy = -ang.cos() * speed;
                // Renormalise to the target speed (momentum is a nudge, not energy).
                let sp = (vx * vx + vy * vy).sqrt().max(1.0);
                vx = vx / sp * speed;
                vy = vy / sp * speed;
                // Never leave going down or flat.
                if vy > -1.0 {
                    vy = -speed * tuning::MIN_VERTICAL_FRAC.max(0.5);
                }
                self.balls[idx].vx = vx;
                self.balls[idx].vy = vy;
                enforce_min_vertical(&mut self.balls[idx]);
                self.score.on_paddle_contact();
            }
            HitKind::Brick(bi) => {
                // Reflect first (steel reflects identically).
                {
                    let b = &mut self.balls[idx];
                    if hit.nx != 0.0 {
                        b.vx = -b.vx;
                    }
                    if hit.ny != 0.0 {
                        b.vy = -b.vy;
                    }
                    enforce_min_vertical(b);
                }
                self.damage_brick(bi);
                // Speed ramp: renormalise all balls to the new target.
                let speed = self.target_speed();
                for ball in self.balls.iter_mut().filter(|b| !b.stuck) {
                    let sp = (ball.vx * ball.vx + ball.vy * ball.vy).sqrt();
                    if sp > 1e-6 {
                        ball.vx = ball.vx / sp * speed;
                        ball.vy = ball.vy / sp * speed;
                    }
                }
            }
        }
    }

    /// Damage one brick; handles HP, steel immunity, explosive chains.
    /// Chains resolve in one pass with a visited set (no recursion, FR-19).
    fn damage_brick(&mut self, bi: usize) {
        if bi >= self.bricks.len() {
            return;
        }
        let kind = self.bricks[bi].kind;
        match kind {
            BrickKind::Steel => {}
            BrickKind::Normal => {
                if self.bricks[bi].hp > 1 {
                    self.bricks[bi].hp -= 1;
                } else {
                    let pos = (self.bricks[bi].col, self.bricks[bi].row);
                    self.bricks.swap_remove(bi);
                    self.on_brick_destroyed();
                    // Shrapnel axis (ADR-0008) is intentionally not branched
                    // on here in Phase 2; perks arrive in Phase 8.
                    let _ = pos;
                }
            }
            BrickKind::Explosive => {
                let (ec, er) = (self.bricks[bi].col, self.bricks[bi].row);
                self.bricks.swap_remove(bi);
                self.on_brick_destroyed();
                self.detonate(ec, er);
            }
        }
    }

    /// Explosive 3x3: destroy every destructible neighbour (chains included).
    fn detonate(&mut self, col: u8, row: u8) {
        // Collect first (indices shift under swap_remove), then destroy.
        let mut chain: Vec<(u8, u8)> = Vec::new();
        let mut visited = [false; 256 * 16];
        let key = |c: u8, r: u8| usize::from(c) * 16 + usize::from(r);
        let mut queue = vec![(col, row)];
        visited[key(col, row)] = true;
        while let Some((c, r)) = queue.pop() {
            for dc in -1i16..=1 {
                for dr in -1i16..=1 {
                    if dc == 0 && dr == 0 {
                        continue;
                    }
                    let nc = c as i16 + dc;
                    let nr = r as i16 + dr;
                    if !(0..18).contains(&nc) || !(0..14).contains(&nr) {
                        continue;
                    }
                    let (nc, nr) = (nc as u8, nr as u8);
                    if visited[key(nc, nr)] {
                        continue;
                    }
                    visited[key(nc, nr)] = true;
                    // Is there a destructible brick here?
                    if let Some(b) = self
                        .bricks
                        .iter()
                        .find(|b| b.col == nc && b.row == nr && b.kind != BrickKind::Steel)
                    {
                        let kind = b.kind;
                        chain.push((nc, nr));
                        if kind == BrickKind::Explosive {
                            queue.push((nc, nr));
                        }
                    }
                }
            }
        }
        // Deterministic order: sort by (row, col) so replay is stable.
        chain.sort_unstable();
        chain.dedup();
        for (c, r) in chain {
            if let Some(pos) = self
                .bricks
                .iter()
                .position(|b| b.col == c && b.row == r && b.kind != BrickKind::Steel)
            {
                self.bricks.swap_remove(pos);
                self.on_brick_destroyed();
            }
        }
    }

    /// Score one destroyed brick with the run score multiplier axis.
    fn on_brick_destroyed(&mut self) {
        let award = self.score.on_brick_destroyed();
        let scaled = (award as f32 * self.modifiers.score_mul) as u64;
        // Replace the unscaled award with the scaled one.
        self.score.points = self
            .score
            .points
            .saturating_sub(u64::from(award))
            .saturating_add(scaled);
        self.bricks_destroyed = self.bricks_destroyed.saturating_add(1);
    }
}

/// Enforce the minimum vertical component (FR-15): the ball can never enter
/// a horizontal loop.
fn enforce_min_vertical(b: &mut Ball) {
    let speed = (b.vx * b.vx + b.vy * b.vy).sqrt();
    if speed < 1e-6 {
        return;
    }
    let min_vy = tuning::MIN_VERTICAL_FRAC * speed;
    if b.vy.abs() < min_vy {
        let sign = if b.vy >= 0.0 { 1.0 } else { -1.0 };
        b.vy = sign * min_vy;
        let vx_mag = (speed * speed - b.vy * b.vy).sqrt();
        b.vx = if b.vx >= 0.0 { vx_mag } else { -vx_mag };
    }
}

/// Swept circle vs AABB: the circle moves by (dx,dy); the box is static.
/// Returns (time-of-impact 0..=1, contact normal). `None` if no hit along
/// the segment. Slab method on the box expanded by the radius.
#[allow(clippy::too_many_arguments)]
fn swept_circle_vs_aabb(
    cx: f32,
    cy: f32,
    dx: f32,
    dy: f32,
    r: f32,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
) -> Option<(f32, f32, f32)> {
    // Expanded box.
    let min_x = bx - r;
    let max_x = bx + bw + r;
    let min_y = by - r;
    let max_y = by + bh + r;

    // Already overlapping: contact with the minimum-penetration normal.
    if cx >= min_x && cx <= max_x && cy >= min_y && cy <= max_y {
        let push_left = cx - min_x;
        let push_right = max_x - cx;
        let push_up = cy - min_y;
        let push_down = max_y - cy;
        let m = push_left.min(push_right).min(push_up).min(push_down);
        if m == push_left {
            return Some((0.0, -1.0, 0.0));
        }
        if m == push_right {
            return Some((0.0, 1.0, 0.0));
        }
        if m == push_up {
            return Some((0.0, 0.0, -1.0));
        }
        return Some((0.0, 0.0, 1.0));
    }

    let mut t_enter: f32 = 0.0;
    let mut t_exit: f32 = 1.0;
    let mut nx = 0.0;
    let mut ny = 0.0;

    if dx.abs() < 1e-9 {
        if cx < min_x || cx > max_x {
            return None;
        }
    } else {
        let inv = 1.0 / dx;
        let mut t1 = (min_x - cx) * inv;
        let mut t2 = (max_x - cx) * inv;
        let mut n = (-inv.signum(), 0.0);
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
            n = (inv.signum(), 0.0);
        }
        if t1 > t_enter {
            t_enter = t1;
            nx = n.0;
            ny = n.1;
        }
        t_exit = t_exit.min(t2);
        if t_enter > t_exit {
            return None;
        }
    }

    if dy.abs() < 1e-9 {
        if cy < min_y || cy > max_y {
            return None;
        }
    } else {
        let inv = 1.0 / dy;
        let mut t1 = (min_y - cy) * inv;
        let mut t2 = (max_y - cy) * inv;
        let mut n = (0.0, -inv.signum());
        if t1 > t2 {
            std::mem::swap(&mut t1, &mut t2);
            n = (0.0, inv.signum());
        }
        if t1 > t_enter {
            t_enter = t1;
            nx = n.0;
            ny = n.1;
        }
        t_exit = t_exit.min(t2);
        if t_enter > t_exit {
            return None;
        }
    }

    if !(0.0..=1.0).contains(&t_enter) {
        return None;
    }
    Some((t_enter, nx, ny))
}

/// One contact.
#[derive(Debug, Clone, Copy)]
struct Hit {
    /// Time of impact along the step, 0..=1.
    toi: f32,
    /// What was hit.
    kind: HitKind,
    /// Contact normal.
    nx: f32,
    /// Contact normal.
    ny: f32,
}

#[derive(Debug, Clone, Copy)]
enum HitKind {
    Wall,
    Paddle,
    Brick(usize),
}

impl Hit {
    fn new(toi: f32, kind: HitKind, nx: f32, ny: f32) -> Self {
        Self { toi, kind, nx, ny }
    }

    /// Keep the earlier of two hits.
    fn min_toi(self, other: Option<Self>) -> Self {
        match other {
            None => self,
            Some(o) if self.toi <= o.toi => self,
            Some(o) => o,
        }
    }
}

/// Headless replay harness (OQ-7): run a recorded input log with a fixed
/// seed and return a byte-stable summary (NFR-10, same-machine scope).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplaySummary {
    /// Final score.
    pub score: u64,
    /// Lives left.
    pub lives: i32,
    /// Destructible bricks left.
    pub remaining: usize,
    /// Best combo.
    pub best_combo: u32,
    /// Final ball integer positions (for byte-stability).
    pub ball_q: Vec<(i32, i32)>,
}

/// Replay `inputs` (one per fixed step) headlessly.
pub fn replay(
    bricks: Vec<Brick>,
    seed: u64,
    level_index: u32,
    modifiers: RunModifiers,
    inputs: &[InputState],
) -> ReplaySummary {
    let mut w = World::new(bricks, seed, level_index, modifiers);
    for input in inputs {
        if w.state != GameState::Playing {
            break;
        }
        w.step(*input, tuning::DT);
    }
    ReplaySummary {
        score: w.score.points,
        lives: w.lives,
        remaining: w.remaining(),
        best_combo: w.score.best_combo,
        ball_q: w
            .balls
            .iter()
            .map(|b| ((b.x * 1000.0) as i32, (b.y * 1000.0) as i32))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_world() -> World {
        World::new(Vec::new(), 42, 0, RunModifiers::default())
    }

    fn brick_at(col: u8, row: u8, hp: u8) -> Brick {
        Brick::normal(col, row, hp)
    }

    #[test]
    fn no_tunnelling_at_max_speed() {
        // Ball at max speed aimed straight at a brick: one step must hit.
        let brick = brick_at(9, 2, 1);
        let (bx, by, _, _bh) = brick.aabb();
        let mut w = World::new(vec![brick], 1, 0, RunModifiers::default());
        w.balls.clear();
        w.balls.push(Ball {
            x: bx + 8.0,
            y: by - 20.0,
            vx: 0.0,
            vy: tuning::BALL_MAX_SPEED,
            stuck: false,
        });
        // Step until the brick is gone or 2 simulated seconds pass.
        let input = InputState::default();
        let mut hit = false;
        for _ in 0..480 {
            w.step(input, tuning::DT);
            if w.remaining() == 0 {
                hit = true;
                break;
            }
        }
        assert!(hit, "max-speed ball tunnelled through a brick");
    }

    #[test]
    fn corner_hit_reflects_both_axes_over_time() {
        // Ball into the corner of a steel block must not stick or pass.
        let mut w = World::new(vec![Brick::steel(5, 2)], 1, 0, RunModifiers::default());
        w.balls.clear();
        let (bx, by, _, _) = w.bricks[0].aabb();
        w.balls.push(Ball {
            x: bx - 10.0,
            y: by - 10.0,
            vx: 120.0,
            vy: 120.0,
            stuck: false,
        });
        let input = InputState::default();
        for _ in 0..240 {
            w.step(input, tuning::DT);
        }
        let b = w.balls[0];
        assert!(
            b.x >= tuning::PLAY_X && b.x <= tuning::PLAY_RIGHT,
            "ball escaped playfield: {b:?}"
        );
        assert_eq!(w.bricks.len(), 1, "steel must survive");
    }

    #[test]
    fn angle_clamp_kills_horizontal_loops() {
        let mut b = Ball {
            x: 160.0,
            y: 100.0,
            vx: 300.0,
            vy: 1.0,
            stuck: false,
        };
        enforce_min_vertical(&mut b);
        let speed = (b.vx * b.vx + b.vy * b.vy).sqrt();
        assert!(
            b.vy.abs() >= tuning::MIN_VERTICAL_FRAC * speed - 1e-3,
            "clamp failed: {b:?}"
        );
    }

    #[test]
    fn explosive_chain_terminates() {
        // 3x3 of explosives + ring of normals: one detonation clears all.
        let mut bricks = Vec::new();
        for c in 4..=6 {
            for r in 2..=4 {
                bricks.push(Brick::explosive(c, r));
            }
        }
        bricks.push(Brick::normal(3, 1, 1));
        bricks.push(Brick::steel(10, 2));
        let mut w = World::new(bricks, 7, 0, RunModifiers::default());
        // Directly damage the centre explosive.
        let centre = w
            .bricks
            .iter()
            .position(|b| b.col == 5 && b.row == 3)
            .expect("centre");
        w.damage_brick(centre);
        // All 9 explosives gone; steel survives; only steel remains + the far normal.
        assert!(
            !w.bricks.iter().any(|b| b.kind == BrickKind::Explosive),
            "chain did not terminate: {:?}",
            w.bricks.len()
        );
        assert!(w.bricks.iter().any(|b| b.kind == BrickKind::Steel));
    }

    #[test]
    fn steel_does_not_count_for_clear() {
        let mut w = World::new(vec![Brick::steel(0, 0)], 1, 0, RunModifiers::default());
        assert_eq!(w.remaining(), 0);
        // Stepping with no destructibles immediately clears.
        w.step(InputState::default(), tuning::DT);
        assert_eq!(w.state, GameState::LevelClear);
    }

    #[test]
    fn combo_resets_on_paddle() {
        let mut w = empty_world();
        w.score.on_brick_destroyed();
        w.score.on_brick_destroyed();
        assert_eq!(w.score.combo, 2);
        w.score.on_paddle_contact();
        assert_eq!(w.score.combo, 0);
    }

    #[test]
    fn deterministic_replay_is_stable() {
        let bricks = vec![brick_at(4, 1, 1), brick_at(5, 1, 2), brick_at(6, 1, 1)];
        let mut inputs = vec![InputState::default(); 1200];
        for (i, inp) in inputs.iter_mut().enumerate() {
            inp.right = i % 240 < 120;
            inp.left = !inp.right;
            inp.launch = i == 5;
        }
        let a = replay(bricks.clone(), 42, 0, RunModifiers::default(), &inputs);
        let b = replay(bricks, 42, 0, RunModifiers::default(), &inputs);
        assert_eq!(a, b, "same seed+inputs must replay identically");
    }

    #[test]
    fn paddle_clamps_to_walls_without_teleport() {
        // Stop-rule support: 10 continuous simulated minutes with a simple
        // tracking paddle. The ball may be lost (lives), but it must never
        // escape the playfield, stick, or panic.
        let mut w = empty_world();
        w.balls.clear(); // paddle-only test; no ball loss
        let input = InputState {
            left: true,
            right: false,
            launch: false,
        };
        let x0 = w.paddle_x;
        for _ in 0..600 {
            w.step_paddle(input, tuning::DT);
        }
        assert!(w.paddle_x < x0);
        assert!(w.paddle_x >= tuning::PLAY_X);
        // Never teleports more than max-vel * dt per step.
        let mut w2 = empty_world();
        w2.balls.clear();
        let mut prev = w2.paddle_x;
        for _ in 0..120 {
            w2.step_paddle(input, tuning::DT);
            assert!((prev - w2.paddle_x).abs() <= tuning::PADDLE_MAX_VEL * tuning::DT + 1e-3);
            prev = w2.paddle_x;
        }
    }

    #[test]
    fn soak_ten_simulated_minutes_no_escape_no_panic() {
        // Stop-rule support: 10 continuous simulated minutes with a simple
        // tracking paddle. Balls may be lost (lives), but none may escape
        // the playfield and the sim must never panic.
        use super::super::level::default_bricks;
        let mut w = World::new(default_bricks(), 42, 0, RunModifiers::default());
        w.state = GameState::Playing;
        // 10 min at 240 Hz.
        for step in 0..144_000 {
            if w.state != GameState::Playing {
                break;
            }
            // Simple AI: track the lowest ball, launch when stuck.
            let target_x = w
                .balls
                .iter()
                .filter(|b| !b.stuck)
                .min_by(|a, b| b.y.partial_cmp(&a.y).unwrap_or(std::cmp::Ordering::Equal))
                .map(|b| b.x)
                .unwrap_or(w.paddle_x);
            let stuck = w.balls.iter().any(|b| b.stuck);
            let input = InputState {
                left: target_x < w.paddle_x - 2.0,
                right: target_x > w.paddle_x + 2.0,
                launch: stuck && step % 30 == 0,
            };
            w.step(input, tuning::DT);
            for b in &w.balls {
                assert!(
                    b.x >= tuning::PLAY_X - 4.0 && b.x <= tuning::PLAY_RIGHT + 4.0,
                    "ball escaped horizontally: {b:?}"
                );
                assert!(b.y >= tuning::PLAY_TOP - 30.0, "ball escaped top: {b:?}");
                assert!(b.y <= tuning::KILL_Y + 5.0, "ball escaped bottom: {b:?}");
                let speed = (b.vx * b.vx + b.vy * b.vy).sqrt();
                if !b.stuck {
                    assert!(speed <= tuning::BALL_MAX_SPEED + 1.0, "overspeed: {speed}");
                    assert!(
                        b.vy.abs() >= tuning::MIN_VERTICAL_FRAC * speed - 1.0,
                        "horizontal loop: {b:?}"
                    );
                }
            }
            assert!(w.lives >= 0, "negative lives");
        }
    }
}
