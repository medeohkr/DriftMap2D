use proteus::particles::Particles;
use proteus::integrators;
use proteus::simulation::{Simulation, SimulationConfig, Integrator};
use proteus::release_manager::{ReleaseConfig, Schedule};

#[test]
fn test_simple_constant_velocity() {
    // Constant eastward current
    let constant_velocity = |_lon: f32, _lat: f32, _depth: f32| (1.0, 0.0);
    
    let config = ReleaseConfig {
        lon: 0.0,
        lat: 0.0,
        schedule: Schedule::Instant,
        total_mass_bq: 1000.0,
        particle_count: 10,
        spread_km: 0.0,
        depth_m: 0.0,
    };
    
    let sim_config = SimulationConfig {
        release_config: config,
        integrator: Integrator::RK4,
        max_particles: 100,
    };
    
    let mut sim = Simulation::new(sim_config);
    
    // Run one step
    sim.update_particles(1.0, 0.0, constant_velocity);
    
    // Check particles moved east
    for i in 0..sim.particles.len {
        assert!(sim.particles.x[i] > 0.0, "Particle should move east");
    }
    
    println!("Test passed! {} particles moved.", sim.particles.len);
}