#[derive(PartialEq, Eq)]
pub enum InterruptType {
    MNI,
}

#[derive(PartialEq, Eq)]
pub struct Interrupt {
    pub itype: InterruptType,
    pub vector_addr: u16,
    pub b_flag_mask: u8,
    pub cpu_cycles: u8,
}

pub const MNI: Interrupt = Interrupt {
    itype: InterruptType::MNI,
    vector_addr: 0xfffa,
    b_flag_mask: 0b00100000,
    cpu_cycles: 2,
};
