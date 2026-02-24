//! Workflow engine implementations.

mod common;
pub mod cromwell;
pub mod cwltool;
pub mod nextflow;
pub mod snakemake;
pub mod tes;

pub use cromwell::CromwellExecutor;
pub use cwltool::CwltoolExecutor;
pub use nextflow::NextflowExecutor;
pub use snakemake::SnakemakeExecutor;
pub use tes::TesExecutorBackend;
