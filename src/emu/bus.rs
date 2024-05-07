use crate::emu::cpu::Mem;
use crate::emu::cartridge::Rom;
use crate::ppu_emu::ppu::{NesPPU, PPU};
use crate::joypad::Joypad;

const RAM: u16 = 0x0000;
const RAM_MIRRORS_END: u16 = 0x1fff;
//const PPU_REGISTERS: u16 = 0x2000;
const PPU_REGISTERS_MIRRORS_END: u16 = 0x3fff;

pub struct Bus<'call> {
    cpu_vram: [u8; 2048],
    rom: Vec<u8>,
    ppu: NesPPU,
    cycles: usize,
    gameloop_callback: Box<dyn FnMut(&NesPPU, &mut Joypad) + 'call>,
    joypad1: Joypad,
}

impl<'a> Bus<'a> {
    pub fn new<'call, F>(rom: Rom, gameloop_callback: F) -> Bus<'call>
    where
        F: FnMut(&NesPPU, &mut Joypad) + 'call,
    {
        let ppu = NesPPU::new(rom.chr_rom, rom.screen_mirroring);
        Bus {
            cpu_vram: [0; 2048],
            rom: rom.prg_rom,
            ppu,
            cycles: 0,
            gameloop_callback: Box::from(gameloop_callback),
            joypad1: Joypad::new(),
        }
    }

    pub fn tick(&mut self, cycles: u8) {
        self.cycles += cycles as usize;

        let nmi_before = self.ppu.nmi_interrupt.is_some();
        self.ppu.tick(cycles * 3);
        let nmi_after = self.ppu.nmi_interrupt.is_some();

        if !nmi_before && nmi_after {
            (self.gameloop_callback)(&self.ppu, &mut self.joypad1);
        }
    }

    fn read_prg_rom(&self, mut addr: u16) -> u8 {
        addr -= 0x8000;
        if self.rom.len() == 0x4000 && addr >= 0x4000 {
            addr = addr % 0x4000;
        }
        self.rom[addr as usize]
    }

    pub fn poll_nmi_status(&mut self) -> Option<u8> {
        self.ppu.poll_nmi_interrupt()
    }
}

impl Mem for Bus<'_> {
    fn mem_read(&mut self, addr: u16) -> u8 {
        match addr {
            RAM..=RAM_MIRRORS_END => {
                let mirror_down_addr = addr & 0b00000111_11111111;
                self.cpu_vram[mirror_down_addr as usize]
            },
            0x2000 | 0x2001 | 0x2003 | 0x2005 | 0x2006 | 0x4014 => {
                0
            },
            0x2002 => self.ppu.read_status(),
            0x2004 => self.ppu.read_oam_data(),
            0x2007 => self.ppu.read_data(),
            0x4000..=0x4015 => {
                0
            },
            0x4016 => {
                self.joypad1.read()
            }

            0x4017 => {
                // ignore joypad 2
                0
            },
            0x2008..=PPU_REGISTERS_MIRRORS_END => {
                let mirror_down_addr = addr & 0b00100000_00000111;
                self.mem_read(mirror_down_addr)
            },
            0x8000..=0xFFFF => self.read_prg_rom(addr),
            _ => {
                println!("Ignoring mem access at {}", addr);
                0
            }
        }
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        match addr {
            RAM..=RAM_MIRRORS_END => {
                let mirror_down_addr = addr & 0b11111111111;
                self.cpu_vram[mirror_down_addr as usize] = data;
            },
            0x2000 => self.ppu.write_to_ctrl(data),
            0x2001 => self.ppu.write_to_mask(data),
            0x2002 => panic!("attempt to write to PPU status register"),
            0x2003 => self.ppu.write_to_oam_addr(data),
            0x2004 => self.ppu.write_to_oam_data(data),
            0x2005 => self.ppu.write_to_scroll(data),
            0x2006 => self.ppu.write_to_ppu_addr(data),
            0x2007 => self.ppu.write_to_data(data),
            0x4000..=0x4013 | 0x4015 => {
                //ignore APU 
            }

            0x4016 => {
                self.joypad1.write(data);
            }

            0x4017 => {
                // ignore joypad 2
            }

            // https://wiki.nesdev.com/w/index.php/PPU_programmer_reference#OAM_DMA_.28.244014.29_.3E_write
            0x4014 => {
                let mut buffer: [u8; 256] = [0; 256];
                let hi: u16 = (data as u16) << 8;
                for i in 0..256u16 {
                    buffer[i as usize] = self.mem_read(hi + i);
                }

                self.ppu.write_oam_dma(&buffer);

                // todo: handle this eventually
                // let add_cycles: u16 = if self.cycles % 2 == 1 { 514 } else { 513 };
                // self.tick(add_cycles); //todo this will cause weird effects as PPU will have 513/514 * 3 ticks
            }
            0x2008..=PPU_REGISTERS_MIRRORS_END => {
                let mirror_down_addr = addr & 0b00100000_00000111;
                self.mem_write(mirror_down_addr, data);
            },
            0x8000..=0xFFFF => {
                panic!("Attempt to write to Cartridge ROM space {:x}", addr)
            },
            _ => println!("Ignoring mem access at {}", addr)

        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::emu::cartridge::test;

    #[test]
    fn test_mem_read_write_to_ram() {
        let mut bus = Bus::new(test::test_rom(), |_ppu: &NesPPU, &mut Joypad| {});
        bus.mem_write(0x01, 0x55);
        assert_eq!(bus.mem_read(0x01), 0x55);
    }
}
