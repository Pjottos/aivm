/// Constants controlling the frequency of different instructions in the VM code.
///
/// A frequency value translates to an estimate percentage of the total instructions which
/// will be compiled as that instruction. The sum of all frequency values must be 2^16 and
/// instructions with a frequency of 0 will never appear in the VM code.
pub trait InstructionFrequencies {
    /// The frequency of the `end_func` instruction.
    const END_FUNC: u16 = 437; // 0.2
    /// The frequency of the `call` instruction.
    const CALL: u16 = 655; // 0.3

    /// The frequency of the `add` instruction.
    const INT_ADD: u16 = 3277; // 1.5
    /// The frequency of the `sub` instruction.
    const INT_SUB: u16 = 1966; // 0.9
    /// The frequency of the `mul` instruction.
    const INT_MUL: u16 = 2840; // 1.3
    /// The frequency of the `mul_high` instruction.
    const INT_MUL_HIGH: u16 = 2185; // 1.0
    /// The frequency of the `mul_high_unsigned` instruction.
    const INT_MUL_HIGH_UNSIGNED: u16 = 2185; // 1.0
    /// The frequency of the `neg` instruction.
    const INT_NEG: u16 = 2185; // 1.0
    /// The frequency of the `abs` instruction.
    const INT_ABS: u16 = 2185; // 1.0
    /// The frequency of the `inc` instruction.
    const INT_INC: u16 = 2185; // 1.0
    /// The frequency of the `dec` instruction.
    const INT_DEC: u16 = 2185; // 1.0
    /// The frequency of the `int_min` instruction.
    const INT_MIN: u16 = 2840; // 1.3
    /// The frequency of the `int_max` instruction.
    const INT_MAX: u16 = 2840; // 1.3

    /// The frequency of the `swap` instruction.
    const BIT_SWAP: u16 = 1092; // 0.5
    /// The frequency of the `or` instruction.
    const BIT_OR: u16 = 2621; // 1.2
    /// The frequency of the `and` instruction.
    const BIT_AND: u16 = 2621; // 1.2
    /// The frequency of the `xor` instruction.
    const BIT_XOR: u16 = 3497; // 1.6
    /// The frequency of the `not` instruction.
    const BIT_NOT: u16 = 2621; // 1.2
    /// The frequency of the `shift_left` instruction.
    const BIT_SHIFT_L: u16 = 2621; // 1.2
    /// The frequency of the `shift_right` instruction.
    const BIT_SHIFT_R: u16 = 2621; // 1.2
    /// The frequency of the `rotate_left` instruction.
    const BIT_ROT_L: u16 = 2621; // 1.2
    /// The frequency of the `rotate_right` instruction.
    const BIT_ROT_R: u16 = 2621; // 1.2
    /// The frequency of the `bit_select` instruction.
    const BIT_SELECT: u16 = 2840; // 1.3
    /// The frequency of the `popcnt` instruction.
    const BIT_POPCNT: u16 = 1966; // 0.9
    /// The frequency of the `bit_reverse` instruction.
    const BIT_REVERSE: u16 = 2403; // 1.1

    /// The frequency of the `branch_cmp` instruction.
    const BRANCH_CMP: u16 = 1092; // 0.5
    /// The frequency of the `branch_zero` instruction.
    const BRANCH_ZERO: u16 = 655; // 0.3
    /// The frequency of the `branch_non_zero` instruction.
    const BRANCH_NON_ZERO: u16 = 655; // 0.3

    /// The frequency of the `load` instruction.
    const MEM_LOAD: u16 = 3276; // 1.5
    /// The frequency of the `store` instruction.
    const MEM_STORE: u16 = 1748; // 0.8

    /// Takes the sum of all frequencies, and subtracts it from 2^16. The result must be 0
    /// or the VM compiler will panic on certain input values.
    ///
    /// Can be used in tests to check if you implemented the trait correctly.
    fn sum_delta() -> i32 {
        (1 << 16)
            - (i32::from(Self::END_FUNC)
                + i32::from(Self::CALL)
                + i32::from(Self::INT_ADD)
                + i32::from(Self::INT_SUB)
                + i32::from(Self::INT_MUL)
                + i32::from(Self::INT_MUL_HIGH)
                + i32::from(Self::INT_MUL_HIGH_UNSIGNED)
                + i32::from(Self::INT_NEG)
                + i32::from(Self::INT_ABS)
                + i32::from(Self::INT_INC)
                + i32::from(Self::INT_DEC)
                + i32::from(Self::INT_MIN)
                + i32::from(Self::INT_MAX)
                + i32::from(Self::BIT_SWAP)
                + i32::from(Self::BIT_OR)
                + i32::from(Self::BIT_AND)
                + i32::from(Self::BIT_XOR)
                + i32::from(Self::BIT_NOT)
                + i32::from(Self::BIT_SHIFT_L)
                + i32::from(Self::BIT_SHIFT_R)
                + i32::from(Self::BIT_ROT_L)
                + i32::from(Self::BIT_ROT_R)
                + i32::from(Self::BIT_SELECT)
                + i32::from(Self::BIT_POPCNT)
                + i32::from(Self::BIT_REVERSE)
                + i32::from(Self::BRANCH_CMP)
                + i32::from(Self::BRANCH_ZERO)
                + i32::from(Self::BRANCH_NON_ZERO)
                + i32::from(Self::MEM_LOAD)
                + i32::from(Self::MEM_STORE))
    }
}

/// The default implementation of [InstructionFrequencies].
pub struct DefaultFrequencies(());

impl InstructionFrequencies for DefaultFrequencies {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_default_sum() {
        assert_eq!(DefaultFrequencies::sum_delta(), 0);
    }
}
