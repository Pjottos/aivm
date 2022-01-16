pub trait InstructionFrequencies {
    const END_FUNC: u16 = 437; // 0.2
    const CALL: u16 = 655; // 0.3

    const INT_ADD: u16 = 3277; // 1.5
    const INT_SUB: u16 = 1966; // 0.9
    const INT_MUL: u16 = 2840; // 1.3
    const INT_MUL_HIGH: u16 = 2185; // 1.0
    const INT_MUL_HIGH_UNSIGNED: u16 = 2185; // 1.0
    const INT_NEG: u16 = 2185; // 1.0
    const INT_ABS: u16 = 2185; // 1.0
    const INT_INC: u16 = 2185; // 1.0
    const INT_DEC: u16 = 2185; // 1.0
    const INT_MIN: u16 = 2840; // 1.3
    const INT_MAX: u16 = 2840; // 1.3

    const BIT_SWAP: u16 = 1092; // 0.5
    const BIT_OR: u16 = 2621; // 1.2
    const BIT_AND: u16 = 2621; // 1.2
    const BIT_XOR: u16 = 3497; // 1.6
    const BIT_NOT: u16 = 2621; // 1.2
    const BIT_SHIFT_L: u16 = 2621; // 1.2
    const BIT_SHIFT_R: u16 = 2621; // 1.2
    const BIT_ROT_L: u16 = 2621; // 1.2
    const BIT_ROT_R: u16 = 2621; // 1.2
    const BIT_SELECT: u16 = 2840; // 1.3
    const BIT_POPCNT: u16 = 1966; // 0.9
    const BIT_REVERSE: u16 = 2403; // 1.1

    const BRANCH_CMP: u16 = 1092; // 0.5
    const BRANCH_ZERO: u16 = 655; // 0.3
    const BRANCH_NON_ZERO: u16 = 655; // 0.3

    const MEM_LOAD: u16 = 3276; // 1.5
    const MEM_STORE: u16 = 1748; // 0.8

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
