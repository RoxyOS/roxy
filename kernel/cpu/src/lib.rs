#![no_std]

mod arch;
mod cpu;
mod local;

pub use cpu::{Cpu, CpuStatistics, current_cpu};
pub use local::CpuLocal;
