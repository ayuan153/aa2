pub use aa2_data;

pub type Tick = u32;

pub const TICK_RATE: f32 = 30.0;
pub const TICK_DURATION: f32 = 1.0 / 30.0;

pub struct Simulation {
    pub tick: Tick,
}

impl Simulation {
    pub fn new() -> Self {
        Self { tick: 0 }
    }

    pub fn step(&mut self) {
        self.tick += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulation_step() {
        let mut sim = Simulation::new();
        assert_eq!(sim.tick, 0);
        sim.step();
        assert_eq!(sim.tick, 1);
    }
}
