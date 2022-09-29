use super::TargetInterface;

pub struct Target {}

impl TargetInterface for Target {
    const MAX_INSTRUCTION_REGS: usize = 4;
    const REGISTER_COUNT: usize = 12;
}
