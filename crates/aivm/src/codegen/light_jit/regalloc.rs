use super::{
    arch::{Target, TargetInterface},
    ir::{Function, LiveRange},
};

use arrayvec::ArrayVec;

use std::fmt::Debug;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PhysicalVar(u32);

impl PhysicalVar {
    const INVALID: Self = Self(u32::MAX);

    fn is_valid(self) -> bool {
        self != Self::INVALID
    }

    fn is_stack(self) -> bool {
        self.0 & 0x80000000 != 0
    }

    fn idx(self) -> u32 {
        self.0 & 0x7FFFFFFF
    }
}

impl Debug for PhysicalVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.is_valid() {
            f.write_str("INVALID")
        } else {
            let name = if self.is_stack() { "Stack" } else { "Reg" };
            f.debug_tuple(name).field(&self.idx()).finish()
        }
    }
}

#[derive(Debug, Default)]
pub struct RegAllocations {
    pub allocations: Vec<ArrayVec<PhysicalVar, { Target::MAX_INSTRUCTION_REGS }>>,
    pub used_regs_mask: u64,
    pub stack_size: u32,
}

impl RegAllocations {
    /// `live_ranges` must be sorted in order of increasing start point
    pub fn run(func: &mut Function, live_ranges: Vec<LiveRange>) {
        let mut live_ranges = live_ranges.into_iter().peekable();
        let mut active_reg: [Option<LiveRange>; Target::REGISTER_COUNT] =
            [None; Target::REGISTER_COUNT];
        let mut used_regs_mask = 0;
        let mut active_stack: [Option<LiveRange>; 64 - Target::REGISTER_COUNT] =
            [None; 64 - Target::REGISTER_COUNT];
        let mut stack_size = 0;

        for (i, inst) in func.instructions.iter().enumerate() {
            let i = i as u32;

            for a in active_reg
                .iter_mut()
                .chain(active_stack.iter_mut())
                .filter(|a| a.map_or(false, |a| a.end >= i))
            {
                *a = None;
            }

            while let Some(new_range) = live_ranges.next_if(|r| r.start == i) {
                if let Some(reg) = active_reg.iter().position(Option::is_none) {
                    used_regs_mask |= 1 << reg;
                    active_reg[reg] = Some(new_range);
                } else {
                    // Spill the variable with the longest remaining lifetime
                    let (r, active_range) = active_reg
                        .iter()
                        .copied()
                        .enumerate()
                        .flat_map(|(r, a)| a.map(|a| (r, a)))
                        .max_by_key(|(_, a)| a.end)
                        .unwrap();

                    let spilled_range = if active_range.end > new_range.end {
                        active_reg[r] = Some(new_range);
                        // TODO: insert move from reg to stack
                        active_range
                    } else {
                        new_range
                    };

                    let stack_idx = active_stack.iter().position(Option::is_none).unwrap();
                    stack_size = stack_size.max(stack_idx as u32 + 1);
                    active_stack[stack_idx] = Some(spilled_range);
                }
            }
        }

        func.reg_allocs = Some(Self {
            allocations: vec![],
            used_regs_mask,
            stack_size,
        });
    }
}
