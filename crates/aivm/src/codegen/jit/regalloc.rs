use super::{
    arch::{Target, TargetInterface},
    ir::{BlockName, Function, InstructionKind, LiveRange, Var},
};

use arrayvec::ArrayVec;

use std::{collections::HashMap, fmt::Debug};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicalVar(u32);

impl PhysicalVar {
    const INVALID: Self = Self(u32::MAX);

    #[inline]
    fn new_register(r: u32) -> Self {
        Self(r & 0x7FFFFFFF)
    }

    #[inline]
    fn new_stack(slot: u32) -> Self {
        Self(slot | 0x80000000)
    }

    #[inline]
    fn is_valid(self) -> bool {
        self != Self::INVALID
    }

    #[inline]
    pub fn is_stack(self) -> bool {
        self.0 & 0x80000000 != 0
    }

    #[inline]
    pub fn idx(self) -> u32 {
        self.0 & 0x7FFFFFFF
    }

    #[inline]
    pub fn offset(self) -> i32 {
        (self.idx() * 8) as i32
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

#[derive(Debug)]
struct State {
    live_vars: HashMap<Var, PhysicalVar>,
    active_reg: [Option<LiveRange>; Target::REGISTER_COUNT],
    active_stack: [Option<LiveRange>; 64 - Target::REGISTER_COUNT],
    stack_size: u32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            live_vars: HashMap::new(),
            active_reg: Default::default(),
            active_stack: [None; 64 - Target::REGISTER_COUNT],
            stack_size: 0,
        }
    }
}

impl State {
    fn clean_dead_vars(&mut self, i: u32) {
        for a in self
            .active_reg
            .iter_mut()
            .chain(self.active_stack.iter_mut())
            .filter(|a| a.map_or(false, |a| a.end == i))
        {
            let range = a.take().unwrap();
            self.live_vars.remove(&range.var);
        }
    }

    fn longest_active_reg(&self) -> Option<(u32, LiveRange)> {
        self.active_reg
            .iter()
            .copied()
            .enumerate()
            .flat_map(|(r, a)| a.map(|a| (r as u32, a)))
            .max_by_key(|(_, a)| a.end)
    }

    fn spill_reg(&mut self, reg: u32, inst: &mut RegAllocInstruction) -> u32 {
        let range = self.active_reg[reg as usize].take().unwrap();
        let stack_idx = self.alloc_stack(range);

        self.active_stack[stack_idx as usize] = Some(range);
        self.reg_to_stack(stack_idx, reg, inst);

        stack_idx
    }

    fn alloc_stack(&mut self, range: LiveRange) -> u32 {
        let stack_idx = self.active_stack.iter().position(Option::is_none).unwrap() as u32;
        self.stack_size = self.stack_size.max(stack_idx + 1);

        self.live_vars
            .insert(range.var, PhysicalVar::new_stack(stack_idx));
        self.active_stack[stack_idx as usize] = Some(range);

        stack_idx
    }

    fn alloc_reg(&mut self, range: LiveRange) -> Option<u32> {
        if let Some(r) = self.active_reg.iter().position(Option::is_none) {
            self.active_reg[r] = Some(range);
            let r = r as u32;
            self.live_vars
                .insert(range.var, PhysicalVar::new_register(r));
            Some(r)
        } else {
            None
        }
    }

    fn use_reg(&mut self, reg: u32, range: LiveRange) {
        let target = &mut self.active_reg[reg as usize];
        debug_assert!(target.is_none());
        self.live_vars
            .insert(range.var, PhysicalVar::new_register(reg));
        *target = Some(range);
    }

    fn unspill(&mut self, stack_idx: u32, inst: &mut RegAllocInstruction) -> u32 {
        let range = self.active_stack[stack_idx as usize].unwrap();

        let reg = if let Some(reg) = self.alloc_reg(range) {
            reg
        } else {
            // Make sure we don't spill a register that's already being used in the current
            // instruction
            let (reg, _) = self
                .active_reg
                .iter()
                .copied()
                .enumerate()
                .flat_map(|(r, a)| a.map(|a| (r as u32, a)))
                .filter(|(r, _)| {
                    let phys = PhysicalVar::new_register(*r);
                    !inst.defs.contains(&phys) && !inst.uses.contains(&phys)
                })
                .max_by_key(|(_, a)| a.end)
                .unwrap();
            self.spill_reg(reg, inst);
            self.use_reg(reg, range);
            reg
        };

        self.stack_to_reg(reg, stack_idx, inst);
        self.active_stack[stack_idx as usize] = None;

        reg
    }

    fn reg_to_stack(&mut self, stack_idx: u32, reg: u32, inst: &mut RegAllocInstruction) {
        // for action in &mut inst.actions {
        //     match action {
        //         RegAllocAction::RegToStack(s, r) if *s == stack_idx => *r = reg,
        //         _ => continue,
        //     }
        //     return;
        // }

        inst.actions
            .push(RegAllocAction::RegToStack(stack_idx, reg));
    }

    fn stack_to_reg(&mut self, reg: u32, stack_idx: u32, inst: &mut RegAllocInstruction) {
        // for action in &mut inst.actions {
        //     match action {
        //         RegAllocAction::StackToReg(r, s) if *r == reg => *s = stack_idx,
        //         _ => continue,
        //     }
        //     return;
        // }

        inst.actions
            .push(RegAllocAction::StackToReg(reg, stack_idx));
    }
}

#[derive(Debug, Default)]
pub struct RegAllocations {
    pub instructions: Vec<RegAllocInstruction>,
    pub used_regs_mask: u64,
    pub stack_size: u32,
}

impl RegAllocations {
    /// `live_ranges` must be sorted in order of increasing start point
    pub fn run(func: &mut Function, live_ranges: Vec<LiveRange>) {
        let allocs = &mut func.reg_allocs;
        allocs.clear();

        let mut live_ranges = live_ranges.into_iter().peekable();
        let mut state = State::default();
        let mut last_block = BlockName::INVALID;

        'func_inst: for (i, (b, func_inst)) in func
            .blocks
            .iter()
            .enumerate()
            .flat_map(|(b, block)| {
                block
                    .instructions
                    .iter()
                    .map(move |i| (BlockName(b as u32), i))
            })
            .enumerate()
        {
            let i = i as u32;

            let mut inst = RegAllocInstruction {
                kind: func_inst.kind,
                actions: vec![],
                defs: ArrayVec::new(),
                uses: ArrayVec::new(),
            };

            state.clean_dead_vars(i);

            while let Some(new_range) = live_ranges.next_if(|r| r.start == i) {
                if let Some(reg) = state.alloc_reg(new_range) {
                    allocs.used_regs_mask |= 1 << reg;
                } else {
                    // Spill the variable with the longest remaining lifetime
                    let (r, active_range) = state.longest_active_reg().unwrap();

                    if active_range.end > new_range.end {
                        state.spill_reg(r, &mut inst);
                        state.use_reg(r, new_range);
                    } else {
                        state.alloc_stack(new_range);
                    };
                }
            }

            // Coalesce split blocks and ignore jump instructions since they always jump
            // to the next block, or the block that the previous block's branch instruction
            // jumps to if the branch is taken.
            match func_inst.kind {
                InstructionKind::Jump => continue,
                InstructionKind::BranchCmp { .. }
                | InstructionKind::BranchZero
                | InstructionKind::BranchNonZero => {
                    let proxy = func.blocks[b.0 as usize].branch_exit;
                    let target = func.blocks[proxy.0 as usize].exit;
                    inst.actions.push(RegAllocAction::BranchExit(target));
                }
                _ => (),
            }

            for (is_dst, virt) in func_inst
                .dst_iter()
                .map(|d| (true, d))
                .chain(func_inst.src_iter().map(|s| (false, s)))
            {
                // A bit hacky, but if live_vars does not contain a referenced variable,
                // that means this instruction is dead and we can discard it
                let mut phys = match state.live_vars.get(&virt) {
                    Some(phys) => *phys,
                    None => continue 'func_inst,
                };

                if phys.is_stack()
                    && (!Target::supports_mem_operand(inst.kind)
                        || inst.defs.iter().any(|v| v.is_stack())
                        || inst.uses.iter().any(|v| v.is_stack()))
                {
                    let reg = state.unspill(phys.idx(), &mut inst);
                    phys = PhysicalVar::new_register(reg);
                }

                if is_dst {
                    inst.defs.push(phys);
                } else {
                    inst.uses.push(phys);
                }
            }

            if b != last_block {
                let start = last_block.0.wrapping_add(1);
                inst.actions
                    .extend((start..=b.0).map(|b| RegAllocAction::BlockStart(BlockName(b))));
            }
            last_block = b;

            allocs.instructions.push(inst);
        }

        allocs.stack_size = state.stack_size;
    }

    fn clear(&mut self) {
        self.instructions.clear();
        self.stack_size = 0;
        self.used_regs_mask = 0;
    }
}

#[derive(Debug)]
pub struct RegAllocInstruction {
    pub kind: InstructionKind,
    pub defs: ArrayVec<PhysicalVar, 1>,
    pub uses: ArrayVec<PhysicalVar, 3>,
    pub actions: Vec<RegAllocAction>,
}

#[derive(Debug)]
pub enum RegAllocAction {
    RegToStack(u32, u32),
    StackToReg(u32, u32),
    BlockStart(BlockName),
    BranchExit(BlockName),
}
