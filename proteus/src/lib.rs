pub mod particles;
pub mod diffusion;
pub mod integrators;
pub mod interpolation;
pub mod data_loader;
pub mod release_manager;
pub mod simulation;
pub mod wasm;
pub mod heatmap;
pub mod landmask_loader;

// Re-export commonly used types for convenience
pub use particles::Particles;
pub use integrators::{euler_step, midpoint_step, rk4_step};
pub use data_loader::{DataLoader, TileKey, TileData, LoaderError};
pub use release_manager::{ReleaseConfig, Schedule, ReleaseManager};
pub use simulation::{Simulation, SimulationConfig, Integrator};