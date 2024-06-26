use std::collections::HashMap;
use crate::emu::opcodes;
use crate::emu::bus::Bus;
use crate::emu::interrupt::*;


pub struct CPU<'a> {
    pub register_a: u8,
    pub register_x: u8,
    pub register_y: u8,
    pub status: u8,
    pub program_counter: u16,
    pub stack_pointer: u8,
    bus: Bus<'a>
}

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum AddressingMode {
    Immediate,
    ZeroPage,
    ZeroPage_X,
    ZeroPage_Y,
    Absolute,
    Absolute_X,
    Absolute_Y,
    Indirect_X,
    Indirect_Y,
    NoneAddressing,
}

pub trait Mem {
    fn mem_read(&mut self, addr: u16) -> u8;

    fn mem_write(&mut self, addr: u16, data: u8);

    fn mem_read_u16(&mut self, pos: u16) -> u16 {
        let lo = self.mem_read(pos) as u16;
        let hi = self.mem_read(pos + 1) as u16;
        (hi << 8) | (lo as u16)
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.mem_write(pos, lo);
        self.mem_write(pos + 1, hi);
    }
}

impl Mem for CPU<'_> {
    fn mem_read(&mut self, addr: u16) -> u8 {
        self.bus.mem_read(addr)
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.bus.mem_write(addr, data)
    }

    fn mem_read_u16(&mut self, pos: u16) -> u16 {
        self.bus.mem_read_u16(pos)
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        self.bus.mem_write_u16(pos, data)
    }


}

impl<'a> CPU<'a> {
    pub fn new<'b>(bus: Bus<'b>) -> CPU<'b> {
        CPU {
            register_a: 0,
            register_x: 0,
            register_y: 0,
            status: 0b0010_0100,
            program_counter: 0,
            stack_pointer: 0xfd,
            bus
        }
    }

    fn page_cross(addr1: u16, addr2: u16) -> bool {
        addr1 & 0xff00 != addr2 & 0xff00
    }

    pub fn get_absolute_address(&mut self, mode: &AddressingMode, addr: u16) -> (u16, bool) {
        match mode {
            AddressingMode::ZeroPage => (self.mem_read(addr) as u16, false),

            AddressingMode::Absolute => (self.mem_read_u16(addr), false),

            AddressingMode::ZeroPage_X => {
                let pos = self.mem_read(addr);
                let addr = pos.wrapping_add(self.register_x) as u16;
                (addr, false)
            }
            AddressingMode::ZeroPage_Y => {
                let pos = self.mem_read(addr);
                let addr = pos.wrapping_add(self.register_y) as u16;
                (addr, false)
            }

            AddressingMode::Absolute_X => {
                let base = self.mem_read_u16(addr);
                let addr = base.wrapping_add(self.register_x as u16);
                (addr, Self::page_cross(base, addr))
            }
            AddressingMode::Absolute_Y => {
                let base = self.mem_read_u16(addr);
                let addr = base.wrapping_add(self.register_y as u16);
                (addr, Self::page_cross(base, addr))
            }

            AddressingMode::Indirect_X => {
                let base = self.mem_read(addr);

                let ptr: u8 = (base as u8).wrapping_add(self.register_x);
                let lo = self.mem_read(ptr as u16);
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                ((hi as u16) << 8 | (lo as u16), false)
            }
            AddressingMode::Indirect_Y => {
                let base = self.mem_read(addr);

                let lo = self.mem_read(base as u16);
                let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
                let deref_base = (hi as u16) << 8 | (lo as u16);
                let deref = deref_base.wrapping_add(self.register_y as u16);
                (deref, Self::page_cross(deref, deref_base))
            }

            _ => {
                panic!("mode {:?} is not supported", mode);
            }
        }
    }

    fn get_operand_address(&mut self, mode: &AddressingMode) -> (u16, bool) {
        match mode {
            AddressingMode::Immediate => (self.program_counter, false),
            _ => self.get_absolute_address(mode, self.program_counter),
        }
    }

    fn set_register_a(&mut self, value: u8) {
        self.register_a = value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn lda(&mut self, mode: &AddressingMode) {
        let (addr, page_cross) = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.set_register_a(value);
        if page_cross {
            self.bus.tick(1);
        }
    }

    fn ldx(&mut self, mode: &AddressingMode) {
        let (addr, page_cross) = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_x = value;
        self.update_zero_and_negative_flags(self.register_x);
        if page_cross {
            self.bus.tick(1);
        }
    }

    fn ldy(&mut self, mode: &AddressingMode) {
        let (addr, page_cross) = self.get_operand_address(mode);
        let value = self.mem_read(addr);
        self.register_y = value;
        self.update_zero_and_negative_flags(self.register_y);
        if page_cross {
            self.bus.tick(1)
        }
    }

    fn sta(&mut self, mode: &AddressingMode) {
        let (addr, _) = self.get_operand_address(mode);
        self.mem_write(addr, self.register_a);
    }

    fn and(&mut self, mode: &AddressingMode) {
        let (addr, page_cross) = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        self.set_register_a(data & self.register_a);
        if page_cross {
            self.bus.tick(1)
        }
    }

    fn eor(&mut self, mode: &AddressingMode) {
        let (addr, page_cross) = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        self.set_register_a(data ^ self.register_a);
        if page_cross {
            self.bus.tick(1)
        }
    }

    fn ora(&mut self, mode: &AddressingMode) {
        let (addr, page_cross) = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        self.set_register_a(data | self.register_a);
        if page_cross {
            self.bus.tick(1)
        }
    }

    fn tax(&mut self) {
        self.register_x = self.register_a;
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn update_zero_and_negative_flags(&mut self, result: u8) {
        if result == 0 {
            self.status = self.status | 0b0000_0010;
        } else {
            self.status = self.status & 0b1111_1101;
        }

        if result & 0b1000_0000 != 0 {
            self.status = self.status | 0b1000_0000;
        } else {
            self.status = self.status & 0b0111_1111;
        }
    }

    fn inc(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let mut data = self.mem_read(addr);
        data = data.wrapping_add(1);
        self.mem_write(addr, data);
        self.update_zero_and_negative_flags(data);
        data
    }

    fn inx(&mut self) {
        self.register_x = self.register_x.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn iny(&mut self) {
        self.register_y = self.register_y.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_y);
    }

    fn dec(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let mut data = self.mem_read(addr);
        data = data.wrapping_sub(1);
        self.mem_write(addr, data);
        self.update_zero_and_negative_flags(data);
        data
    }

    fn dex(&mut self) {
        self.register_x = self.register_x.wrapping_sub(1);
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn dey(&mut self) {
        self.register_y = self.register_y.wrapping_sub(1);
        self.update_zero_and_negative_flags(self.register_y);
    }

    pub fn load_and_run(&mut self, program: Vec<u8>) {
        self.load(program);
        self.reset();
        self.program_counter = 0x0600;
        self.run()
    }

    pub fn reset(&mut self) {
        self.register_a = 0;
        self.register_x = 0;
        self.register_y = 0;
        self.stack_pointer = 0xfd;
        self.status = 0b0010_0100;

        self.program_counter = self.mem_read_u16(0xFFFC);
    }

    pub fn load(&mut self, program: Vec<u8>) {
        for i in 0..(program.len() as u16) {
            self.mem_write(0x0600 + i, program[i as usize]);
        }
        //self.mem_write_u16(0xFFFC, 0x0600);
        //self.memory[0x0600..(0x0600 + program.len())].copy_from_slice(&program[..]);
        //self.mem_write_u16(0xFFFC, 0x0600);
    }

    fn set_carry_flag(&mut self) {
        self.status = self.status | 0b0000_0001
    }

    fn clear_carry_flag(&mut self) {
        self.status = self.status & 0b1111_1110
    }

    fn adc(&mut self, mode: &AddressingMode) {
        let (addr, page_cross) = self.get_operand_address(mode);
        let data = self.mem_read(addr);

        let a = self.register_a.clone();
        let c = self.status & 0b0000_0001;
        let sum = a as u16 + data as u16 + c as u16;

        if sum > 0xFF {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }

        let result = sum as u8;
        if (data ^ result) & (result ^ self.register_a) & 0x80 != 0 {
            self.status = self.status | 0b0100_0000;
        } else {
            self.status = self.status & 0b1011_1111;
        }

        self.set_register_a(result);
        if page_cross {
            self.bus.tick(1)
        }
    }

    fn sbc(&mut self, mode: &AddressingMode) {
        let (addr, page_cross) = self.get_operand_address(mode);
        let data = self.mem_read(addr);

        let a = self.register_a.clone();
        let b = (data as i8).wrapping_neg().wrapping_sub(1) as u8;
        let c = self.status & 0b0000_0001;

        let sum = a as u16 + b as u16 + c as u16;

        if sum > 0xFF {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }

        let result = sum as u8;
        if (b ^ result) & (result ^ self.register_a) & 0x80 != 0 {
            self.status = self.status | 0b0100_0000;
        } else {
            self.status = self.status & 0b1011_1111;
        }

        self.set_register_a(result);
        if page_cross {
            self.bus.tick(1)
        }
    }

    fn stack_pop(&mut self) -> u8 {
        self.stack_pointer = self.stack_pointer.wrapping_add(1);
        self.mem_read(0x0100 + self.stack_pointer as u16)
    }

    fn stack_push(&mut self, data: u8) {
        self.mem_write(0x0100 + self.stack_pointer as u16, data);
        self.stack_pointer = self.stack_pointer.wrapping_sub(1);
    }

    fn stack_pop_u16(&mut self) -> u16 {
        let lo = self.stack_pop() as u16;
        let hi = self.stack_pop() as u16;
        hi << 8 | lo
    }

    fn stack_push_u16(&mut self, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;

        self.stack_push(hi);
        self.stack_push(lo);
    }

    fn pla(&mut self) {
        let data = self.stack_pop();
        self.set_register_a(data);
    }

    fn plp(&mut self) {
        self.status = self.stack_pop();
        self.status = self.status & 0b1110_1111;
        self.status = self.status | 0b0010_0000;
    }

    fn php(&mut self) {
        let flag = self.status | 0b0011_0000;
        self.stack_push(flag);
    }

    fn bit(&mut self, mode: &AddressingMode) {
        let (addr, _) = self.get_operand_address(mode);
        let data = self.mem_read(addr);

        if (data & 0b0100_0000) >> 6 == 1 {
            self.status = self.status | 0b0100_0000;
        } else {
            self.status = self.status & 0b1011_1111;
        }

        if data >> 7 == 1 {
            self.status = self.status | 0b1000_0000;
        } else {
            self.status = self.status & 0b0111_1111;
        }

        if self.register_a & data == 0 {
            self.status = self.status | 0b0000_0010;
        } else {
            self.status = self.status & 0b1111_1101;
        }
    }

    fn compare(&mut self, mode: &AddressingMode, compare_with: u8) {
        let (addr, page_cross) = self.get_operand_address(mode);
        let data = self.mem_read(addr);

        if data <= compare_with {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }

        self.update_zero_and_negative_flags(compare_with.wrapping_sub(data));
        if page_cross {
            self.bus.tick(1)
        }
    }

    fn branch(&mut self, condition: bool) {
        if condition {
            self.bus.tick(1);
            let jump: i8 = self.mem_read(self.program_counter) as i8;
            let jump_addr = self.program_counter.wrapping_add(1).wrapping_add(jump as u16);

            if self.program_counter.wrapping_add(1) & 0xff00 != jump_addr & 0xff00 {
                self.bus.tick(1);
            }
            self.program_counter = jump_addr;
        }
    }

    fn asl_accumulator(&mut self) {
        let data = self.register_a;
        if data >> 7 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }
        self.set_register_a(data << 1)
    }

    fn asl(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        if data >> 7 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }
        self.mem_write(addr, data << 1);
        self.update_zero_and_negative_flags(data);
        data
    }

    fn lsr_accumulator(&mut self) {
        let data = self.register_a;
        if data & 1 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }
        self.set_register_a(data >> 1)
    }

    fn lsr(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let data = self.mem_read(addr);
        if data & 1 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }
        self.mem_write(addr, data >> 1);
        self.update_zero_and_negative_flags(data);
        data
    }

    fn rol(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let mut data = self.mem_read(addr);
        let old_array = self.status & 0b0000_0001 == 1;

        if data >> 7 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }

        data = data << 1;
        if old_array {
            data = data | 1;
        }
        self.mem_write(addr, data);
        self.update_zero_and_negative_flags(data);
        data
    }

    fn rol_accumulator(&mut self) {
        let mut data = self.register_a;
        let old_array = self.status & 0b0000_0001 == 1;

        if data >> 7 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }

        data = data << 1;
        if old_array {
            data = data | 1;
        }
        self.set_register_a(data)
    }

    fn ror(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, _) = self.get_operand_address(mode);
        let mut data = self.mem_read(addr);
        let old_array = self.status & 0b0000_0001 == 1;

        if data & 1 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }

        data = data >> 1;
        if old_array {
            data = data | 0b1000_0000;
        }
        self.mem_write(addr, data);
        self.update_zero_and_negative_flags(data);
        data
    }

    fn ror_accumulator(&mut self) {
        let mut data = self.register_a;
        let old_array = self.status & 0b0000_0001 == 1;

        if data & 1 == 1 {
            self.set_carry_flag();
        } else {
            self.clear_carry_flag();
        }

        data = data >> 1;
        if old_array {
            data = data | 0b1000_0000;
        }
        self.set_register_a(data)
    }

    fn unofficial_isb(&mut self, mode: &AddressingMode) {
        let data = self.inc(mode);

        let a = self.register_a.clone();
        let b = (data as i8).wrapping_neg().wrapping_sub(1) as u8;
        let c = self.status & 0b0000_0001;

        let sum = a as u16 + b as u16 + c as u16;

        if sum > 0xFF {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }

        let result = sum as u8;
        if (b ^ result) & (result ^ self.register_a) & 0x80 != 0 {
            self.status = self.status | 0b0100_0000;
        } else {
            self.status = self.status & 0b1011_1111;
        }

        self.register_a = result;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn unofficial_slo(&mut self, mode: &AddressingMode) {
        let data = self.asl(mode);
        self.register_a = data | self.register_a;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn unofficial_rla(&mut self, mode: &AddressingMode) {
        let data = self.rol(mode);
        self.register_a = data & self.register_a;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn unofficial_sre(&mut self, mode: &AddressingMode) {
        let data = self.lsr(mode);
        self.register_a = data ^ self.register_a;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn unofficial_rra(&mut self, mode: &AddressingMode) {
        let data = self.ror(mode);

        let a = self.register_a.clone();
        let c = self.status & 0b0000_0001;
        let sum = a as u16 + data as u16 + c as u16;

        if sum > 0xff {
            self.status = self.status | 0b0000_0001;
        } else {
            self.status = self.status & 0b1111_1110;
        }

        let result = sum as u8;
        if (data ^ result) & (result ^ self.register_a) & 0x80 != 0 {
            self.status = self.status | 0b0100_0000;
        } else {
            self.status = self.status & 0b1011_1111;
        }

        self.register_a = result;
        self.update_zero_and_negative_flags(self.register_a);
    }

    pub fn run(&mut self) {
        self.run_with_callback(|_| {});
    }

    pub fn run_with_callback<F>(&mut self, mut callback: F)
    where
        F: FnMut(&mut CPU),
    {
        let ref opcodes: HashMap<u8, &'static opcodes::OpCode> = *opcodes::OPECODES_MAP;

        loop {
            if let Some(_nmi) = self.bus.poll_nmi_status() {
                self.interrupt(MNI);
            }
            callback(self);
            let code = self.mem_read(self.program_counter);
            self.program_counter += 1;
            let program_counter_state = self.program_counter;

            let opcode = opcodes.get(&code).expect(&format!("OpCode {:x} is not recognized", code));

            match code {
                //LDA
                0xa9 | 0xa5 | 0xb5 | 0xad | 0xbd | 0xb9 | 0xa1 | 0xb1 => self.lda(&opcode.mode),
                //CLD
                0xd8 => self.status = self.status & 0b1111_0111,
                //CLI
                0x58 => self.status = self.status & 0b1111_1011, 
                //CLV
                0xb8 => self.status = self.status & 0b1011_1111,
                //CLC
                0x18 => self.clear_carry_flag(),
                //SEC
                0x38 => self.set_carry_flag(),
                //SEI
                0x78 => self.status = self.status | 0b0000_0100,
                //SED
                0xf8 => self.status = self.status | 0b0000_1000,
                //PHA
                0x48 => self.stack_push(self.register_a),
                //PLA
                0x68 => self.pla(),
                //PHP
                0x08 => self.php(),
                //PLP
                0x28 => self.plp(),
                //ADC
                0x69 | 0x65 | 0x75 | 0x6d | 0x7d | 0x79 | 0x61 | 0x71 => self.adc(&opcode.mode),
                //SBC
                0xe9 | 0xe5 | 0xf5 | 0xed | 0xfd | 0xf9 | 0xe1 | 0xf1 => self.sbc(&opcode.mode),
                //AND
                0x29 | 0x25 | 0x35 | 0x2d | 0x3d | 0x39 | 0x21 | 0x31 => self.and(&opcode.mode),
                //EOR
                0x49 | 0x45 | 0x55 | 0x4d | 0x5d | 0x59 | 0x41 | 0x51 => self.eor(&opcode.mode),
                //ORA
                0x09 | 0x05 | 0x15 | 0x0d | 0x1d | 0x19 | 0x01 | 0x11 => self.ora(&opcode.mode),
                //LSR
                0x4a => self.lsr_accumulator(),
                0x46 | 0x56 | 0x4e | 0x5e => {
                    self.lsr(&opcode.mode);
                },
                //ASL
                0x0a => self.asl_accumulator(),
                0x06 | 0x16 | 0x0e | 0x1e => {
                    self.asl(&opcode.mode);
                },
                //ROL
                0x2a => self.rol_accumulator(),
                0x26 | 0x36 | 0x2e | 0x3e => {
                    self.rol(&opcode.mode);
                },
                //ROR
                0x6a => self.ror_accumulator(),
                0x66 | 0x76 | 0x6e | 0x7e => {
                    self.ror(&opcode.mode);
                },
                //INC
                0xe6 | 0xf6 | 0xee | 0xfe => {
                    self.inc(&opcode.mode);
                },
                //INX
                0xE8 => self.inx(),
                //INY
                0xc8 => self.iny(),
                //DEC
                0xc6 | 0xd6 | 0xce | 0xde => {
                    self.dec(&opcode.mode);
                },
                //DEX
                0xca => self.dex(),
                //DEY
                0x88 => self.dey(),
                //CMP
                0xc9 | 0xc5 | 0xd5 | 0xcd | 0xdd | 0xd9 | 0xc1 | 0xd1 => self.compare(&opcode.mode, self.register_a),
                //CPY
                0xc0 | 0xc4 | 0xcc => self.compare(&opcode.mode, self.register_y),
                //CPX
                0xe0 | 0xe4 | 0xec => self.compare(&opcode.mode, self.register_x),
                //JMP Absolute
                0x4c => {
                    let mem_addr = self.mem_read_u16(self.program_counter);
                    self.program_counter = mem_addr;
                },
                //JMP Indirect_X
                0x6c => {
                    let mem_addr = self.mem_read_u16(self.program_counter);
                    let indirect_ref = if mem_addr & 0x00ff == 0x00ff {
                        let lo = self.mem_read(mem_addr);
                        let hi = self.mem_read(mem_addr & 0xff00);
                        (hi as u16) << 8 | (lo as u16)
                    } else {
                        self.mem_read_u16(mem_addr)
                    };
                    self.program_counter = indirect_ref;
                },
                //JSR
                0x20 => {
                    self.stack_push_u16(self.program_counter + 2 - 1);
                    let target_addr = self.mem_read_u16(self.program_counter);
                    self.program_counter = target_addr
                },
                //RTS
                0x60 => self.program_counter = self.stack_pop_u16() + 1,
                //RTI
                0x40 => {
                    self.status = self.stack_pop();
                    self.status = self.status & 0b1110_1111;
                    self.status = self.status | 0b0010_0000;
                    self.program_counter = self.stack_pop_u16();

                },
                //BNE
                0xd0 => self.branch(self.status & 0b0000_0010 == 0),
                //BVS
                0x70 => self.branch(self.status & 0b0100_0000 != 0),
                //BVC
                0x50 => self.branch(self.status & 0b0100_0000 == 0),
                //BPL
                0x10 => self.branch(self.status & 0b1000_0000 == 0),
                //BMI
                0x30 => self.branch(self.status & 0b1000_0000 != 0),
                //BEQ
                0xf0 => self.branch(self.status & 0b0000_0010 != 0),
                //BCS
                0xb0 => self.branch(self.status & 0b0000_0001 != 0),
                //BCC
                0x90 => self.branch(self.status & 0b0000_0001 == 0),
                //BIT
                0x24 | 0x2c => self.bit(&opcode.mode),
                //STA
                0x85 | 0x95 | 0x8d | 0x9d | 0x99 | 0x81 | 0x91 => self.sta(&opcode.mode),
                //STX
                0x86 | 0x96 | 0x8e => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    self.mem_write(addr, self.register_x);
                },
                //STY
                0x84 | 0x94 | 0x8c => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    self.mem_write(addr, self.register_y);
                },
                //LDX
                0xa2 | 0xa6 | 0xb6 | 0xae | 0xbe => self.ldx(&opcode.mode),
                //LDY
                0xa0 | 0xa4 | 0xb4 | 0xac | 0xbc => self.ldy(&opcode.mode),
                //TAX
                0xaa => self.tax(),
                //TAY
                0xa8 => {
                    self.register_y = self.register_a;
                    self.update_zero_and_negative_flags(self.register_y);
                },
                //TSX
                0xba =>{
                    self.register_x = self.stack_pointer;
                    self.update_zero_and_negative_flags(self.register_x);
                },
                //TXA
                0x8a => {
                    self.register_a = self.register_x;
                    self.update_zero_and_negative_flags(self.register_a);
                },
                //TXS
                0x9a => self.stack_pointer = self.register_x,
                //TYA
                0x98 => {
                    self.register_a = self.register_y;
                    self.update_zero_and_negative_flags(self.register_a);
                },
                //NOP
                0xea => {},
                //BRK
                0x00 => return,
                //unofficial opcodes
                //NOPS
                0x04 | 0x44 | 0x64 | 0x14 | 0x34 | 0x54 | 0x74 | 0xd4 | 0xf4 | 0x0c | 0x1c | 0x3c | 0x5c | 0x7c | 0xdc | 0xfc => {
                    let (_, page_cross) = self.get_operand_address(&opcode.mode);
                    if page_cross {
                        self.bus.tick(1)
                    }
                },
                0x02 | 0x12 | 0x22 | 0x32 | 0x42 | 0x52 | 0x62 | 0x72 | 0x92 | 0xb2 | 0xd2 | 0xf2 => {},
                0x1a | 0x3a | 0x5a | 0x7a | 0xda | 0xfa => {},
                0x80 | 0x82 | 0x89 | 0xc2 | 0xe2 => {},
                //LAX
                0xa7 | 0xb7 | 0xaf | 0xbf | 0xa3 | 0xb3 => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let data = self.mem_read(addr);
                    self.register_a = data;
                    self.update_zero_and_negative_flags(self.register_a);
                    self.register_x = self.register_a;
                },
                //SAX
                0x87 | 0x97 | 0x8f | 0x83 => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let data = self.register_a & self.register_x;
                    self.mem_write(addr, data);
                },
                //SBC
                0xeb => self.sbc(&opcode.mode),
                //DCP
                0xc7 | 0xd7 | 0xcf | 0xdf | 0xdb | 0xd3 | 0xc3 => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let mut data = self.mem_read(addr);
                    data = data.wrapping_add(1);
                    self.mem_write(addr, data);

                    if data <= self.register_a {
                        self.status = self.status | 0x0000_0001;
                    }
                    self.update_zero_and_negative_flags(self.register_a.wrapping_sub(data));
                },
                //ISB
                0xe7 | 0xf7 | 0xef | 0xff | 0xfb | 0xe3 | 0xf3 => self.unofficial_isb(&opcode.mode),
                //SLO
                0x07 | 0x17 | 0x0f | 0x1f | 0x1b | 0x03 | 0x13 => self.unofficial_slo(&opcode.mode),
                //RLA
                0x27 | 0x37 | 0x2F | 0x3F | 0x3b | 0x33 | 0x23 => self.unofficial_rla(&opcode.mode),
                //SRE
                0x47 | 0x57 | 0x4f | 0x5f | 0x5b | 0x43 | 0x53 => self.unofficial_sre(&opcode.mode),
                //RRA
                0x67 | 0x77 | 0x6f | 0x7f | 0x7b | 0x63 | 0x73 => self.unofficial_rra(&opcode.mode),
                //AXS
                0xcb => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let data = self.mem_read(addr);
                    let x_and_a = self.register_x & self.register_a;
                    let result = x_and_a.wrapping_sub(data);

                    if data <= x_and_a {
                        self.status = self.status | 0b0000_0001;
                    }
                    self.update_zero_and_negative_flags(result);

                    self.register_x = result;
                },
                //ARR
                0x6b => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let data = self.mem_read(addr);

                    self.register_a = data & self.register_a;
                    self.update_zero_and_negative_flags(self.register_a);
                    self.ror_accumulator();

                    let result = self.register_a;
                    let bit_5 = (result >> 5) & 1;
                    let bit_6 = (result >> 6) & 1;

                    if bit_6 == 1 {
                        self.status = self.status | 0b0000_0001;
                    } else {
                        self.status = self.status & 0b1111_1110;
                    }

                    if bit_5 ^ bit_6 == 1 {
                        self.status = self.status | 0b0100_0000;
                    } else {
                        self.status = self.status & 0b1011_1111;
                    }

                    self.update_zero_and_negative_flags(result);
                },
                //ANC
                0x0b | 0x2b => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let data = self.mem_read(addr);
                    self.register_a = data & self.register_a;
                    self.update_zero_and_negative_flags(self.register_a);
                    if self.status == 0b1000_0000 {
                        self.status = self.status | 0b0000_0001;
                    } else {
                        self.status = self.status & 0b1111_1110;
                    }
                },
                //ALR
                0x4b => {
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let data = self.mem_read(addr);
                    self.register_a = data & self.register_a;
                    self.update_zero_and_negative_flags(self.register_a);
                    self.lsr_accumulator();
                },
                //LXA
                0xab => {
                    self.lda(&opcode.mode);
                    self.tax();
                },
                //XAA
                0x8b => {
                    self.register_a = self.register_x;
                    self.update_zero_and_negative_flags(self.register_a);
                    let (addr, _) = self.get_operand_address(&opcode.mode);
                    let data = self.mem_read(addr);
                    self.register_a = data & self.register_a;
                    self.update_zero_and_negative_flags(self.register_a);
                },
                //LAS
                0x9b => {
                    let data = self.register_a & self.register_x;
                    self.stack_pointer = data;
                    let mem_address = self.mem_read_u16(self.program_counter) + self.register_y as u16;

                    let data = ((mem_address >> 8) as u8 + 1) & self.stack_pointer;
                    self.mem_write(mem_address, data);
                },
                //AHX I Y
                0x93 => {
                    let pos: u8 = self.mem_read(self.program_counter);
                    let mem_address = self.mem_read_u16(pos as u16) + self.register_y as u16;
                    let data = self.register_a & self.register_x & (mem_address >> 8) as u8;
                    self.mem_write(mem_address, data);
                },
                //AHX A X
                0x9f => {
                    let mem_address = self.mem_read_u16(self.program_counter) + self.register_y as u16;
                    let data = self.register_a & self.register_x & (mem_address >> 8) as u8;
                    self.mem_write(mem_address, data);
                },
                //SHX
                0x9e => {
                    let mem_address = self.mem_read_u16(self.program_counter) + self.register_x as u16;
                    let data = self.register_y & ((mem_address >> 8) as u8 + 1);
                    self.mem_write(mem_address, data);
                }
                _ => todo!()
            }

            self.bus.tick(opcode.cycles);
            if program_counter_state == self.program_counter {
                self.program_counter += (opcode.len - 1) as u16;
            }

        }
    }

    fn interrupt(&mut self, interrupt: Interrupt) {
        self.stack_push_u16(self.program_counter);
        let mut flag = self.status.clone();
        if interrupt.b_flag_mask & 0b010000 == 1 {
            flag = flag | 0b0001_0000;
        } else {
            flag = flag & 0b1110_1111;
        }
        if interrupt.b_flag_mask & 0b100000 == 1 {
            flag = flag | 0b0010_0000;
        } else {
            flag = flag & 0b1101_1111;
        }

        self.stack_push(flag);
        self.status = self.status | 0b0000_0100;

        self.bus.tick(interrupt.cpu_cycles);
        self.program_counter = self.mem_read_u16(interrupt.vector_addr);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::emu::cartridge::test;
    use crate::ppu_emu::ppu::NesPPU;

    #[test]
    fn test_0xa9_lda_immediate_load_data() {
        let bus = Bus::new(test::test_rom(), |ppu: &NesPPU, &mut Joypad| {});
        let mut cpu = CPU::new(bus);
        cpu.load_and_run(vec![0xa9, 0x05, 0x00]);
        assert_eq!(cpu.register_a, 5);
        assert!(cpu.status & 0b0000_0010 == 0b00);
        assert!(cpu.status & 0b1000_0000 == 0);
    }

    #[test]
    fn test_0xaa_tax_move_a_to_x() {
        let bus = Bus::new(test::test_rom(), |ppu: &NesPPU, &mut Joypad| {});
        let mut cpu = CPU::new(bus);
        cpu.register_a = 10;
        cpu.load_and_run(vec![0xa9, 0x0A,0xaa, 0x00]);

        assert_eq!(cpu.register_x, 10)
    }

    #[test]
    fn test_5_ops_working_together() {
        let bus = Bus::new(test::test_rom(), |ppu: &NesPPU, &mut Joypad| {});
        let mut cpu = CPU::new(bus);
        cpu.load_and_run(vec![0xa9, 0xc0, 0xaa, 0xe8, 0x00]);

        assert_eq!(cpu.register_x, 0xc1)
    }

    #[test]
    fn test_inx_overflow() {
        let bus = Bus::new(test::test_rom(), |ppu: &NesPPU, &mut Joypad| {});
        let mut cpu = CPU::new(bus);
        cpu.load_and_run(vec![0xa2, 0xff, 0xe8, 0xe8, 0x00]);

        assert_eq!(cpu.register_x, 1)
    }

    #[test]
    fn test_lda_from_memory() {
        let bus = Bus::new(test::test_rom(), |ppu: &NesPPU, &mut Joypad| {});
        let mut cpu = CPU::new(bus);
        cpu.mem_write(0x10, 0x55);

        cpu.load_and_run(vec![0xa5, 0x10, 0x00]);

        assert_eq!(cpu.register_a, 0x55);
    }
}
