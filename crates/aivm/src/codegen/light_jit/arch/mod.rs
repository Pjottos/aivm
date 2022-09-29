use dynasmrt::{relocations, DynasmApi};

#[cfg(target_arch = "x86_64")]
mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use x86_64::Target;

#[cfg(not(any(target_arch = "x86_64")))]
compile_error!("unsupported architecture for light_jit");

pub trait TargetInterface {
    type Relocation: relocations::Relocation;

    const MAX_INSTRUCTION_REGS: usize;
    const REGISTER_COUNT: usize;

    fn emit_prologue<A: DynasmApi>(ops: &mut A, stack_size: u32, used_regs_mask: u64);
    fn emit_epilogue<A: DynasmApi>(ops: &mut A, stack_size: u32, used_regs_mask: u64);
}
