use proteus::particles::Particles;
use proteus::integrators;
use proteus::simulation::{Simulation, SimulationConfig, Integrator};

#[test]
fn test_simple_constant_velocity() {
    // Constant eastward current
    let constant_velocity = |_lon: f32, _lat: f32| (1.0, 0.0);
    
    let sim_config = SimulationConfig {
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