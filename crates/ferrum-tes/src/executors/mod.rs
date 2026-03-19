mod noop;
mod podman;
mod slurm;

pub use noop::NoopExecutor;
pub use podman::PodmanExecutor;
pub use slurm::SlurmExecutor;

#[cfg(feature = "docker")]
mod docker;

#[cfg(feature = "docker")]
pub use docker::DockerExecutor;
