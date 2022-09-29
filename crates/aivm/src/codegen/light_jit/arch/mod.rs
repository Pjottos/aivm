#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::Target;

#[cfg(not(any(target_arch = "x86_64")))]
compile_error!("unsupported architecture for light_jit");

pub trait TargetInterface {
    const MAX_INSTRUCTION_REGS: usize;
    const REGISTER_COUNT: usize;
}
