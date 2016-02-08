use std::fmt;
use mem::Memory as Mem;
use std::num::Wrapping as W;


macro_rules! set_overflow {
    ($flags:expr) => ($flags = $flags | W(1 << 6));
}

macro_rules! unset_overflow {
    ($flags:expr) => ($flags = $flags & !W(1 << 6));
}

macro_rules! uset_negative {
    ($flags:expr, $val:expr) => ( 
        if ($val & (1 << 7)) == 0 {
            $flags = $flags & !W(1 << 7)
        }else{
            $flags = $flags | W(1 << 7)
        });
}

macro_rules! set_break {
    ($flags:expr) => ($flags = $flags | W(1 << 4));
}

macro_rules! unset_break {
    ($flags:expr) => ($flags = $flags & !W(1 << 4));
}

macro_rules! set_decimal {
    ($flags:expr) => ($flags = $flags | W(1 << 3));
}

macro_rules! unset_decimal {
    ($flags:expr) => ($flags = $flags & !W(1 << 3));
}

macro_rules! set_interrupt {
    ($flags:expr) => ($flags = $flags | W(1 << 2));
}

macro_rules! unset_interrupt {
    ($flags:expr) => ($flags = $flags & !W(1 << 2));
}

macro_rules! set_zero {
    ($flags:expr) => ($flags = $flags | W(1 << 1));
}

macro_rules! unset_zero {
    ($flags:expr) => ($flags = $flags & !W(1 << 1));
}

macro_rules! set_carry {
    ($flags:expr) => ($flags = $flags | W(1));
}

macro_rules! unset_carry {
    ($flags:expr) => ($flags = $flags & !W(1));
}

const OP_SPECIAL_TABLE : [fn(&mut CPU, &mut Mem) -> (); 4] = [
    CPU::brk,
    CPU::invalid,
    CPU::rti,
    CPU::rts,
];

const OP_BRANCH_TABLE : [fn(&mut CPU, &mut Mem, i8) -> (); 8] = [
    CPU::bpl,
    CPU::bmi,
    CPU::bvc,
    CPU::bvs,
    CPU::bcc,
    CPU::bcs,
    CPU::bne,
    CPU::beq,
];

const OP_IMPLIED_TABLE : [fn(&mut CPU, &mut Mem) -> (); 32] = [
    CPU::php,
    CPU::asl_a,
    CPU::clc,
    CPU::invalid,
    CPU::plp, 
    CPU::rol_a,
    CPU::sec,
    CPU::invalid,
    CPU::pha,
    CPU::lsr_a,
    CPU::cli,
    CPU::invalid,
    CPU::pla,
    CPU::ror_a,
    CPU::sei,
    CPU::invalid,
    CPU::dey,
    CPU::txa,
    CPU::tya,
    CPU::txs,
    CPU::tay,
    CPU::tax,
    CPU::clv,
    CPU::tsx,
    CPU::iny,
    CPU::dex,
    CPU::cld,
    CPU::invalid,
    CPU::inx,
    CPU::nop,
    CPU::sed,
    CPU::invalid,
];

const OP_COMMON_TABLE : [fn(&mut CPU, &mut Mem, u8) -> (); 32] = [
    CPU::invalid_c,
    CPU::ora,
    CPU::asl,
    CPU::invalid_c,
    CPU::bit,
    CPU::and,
    CPU::rol,
    CPU::invalid_c,
    CPU::invalid_c,
    CPU::eor,
    CPU::lsr,
    CPU::invalid_c,
    CPU::invalid_c,
    CPU::adc,
    CPU::ror,
    CPU::invalid_c,
    CPU::sty,
    CPU::sta,
    CPU::stx,
    CPU::invalid_c,
    CPU::ldy,
    CPU::lda,
    CPU::ldx,
    CPU::invalid_c,
    CPU::cpy,
    CPU::cmp,
    CPU::dec,
    CPU::invalid_c,
    CPU::cpx,
    CPU::sbc,
    CPU::inc,
    CPU::invalid_c,
];

const OP_JUMP_MASK     : u8 = 0xDF;
const OP_JUMP          : u8 = 0x4C;
const OP_SPECIAL_MASK  : u8 = 0x9F;
const OP_SPECIAL       : u8 = 0x00;
const OP_BRANCH_MASK   : u8 = 0x1F;
const OP_BRANCH        : u8 = 0x10;
const OP_IMPLIED_MASK  : u8 = 0x1F;
const OP_IMPLIED       : u8 = 0x08;
const OP_JSR           : u8 = 0x20;

const STACK_PAGE       : u16 = 0x0100;

#[allow(non_snake_case)]
pub struct CPU {
    A : W<u8>,  // Accumulator
    X : W<u8>,  // Indexes
    Y : W<u8>,  
    Flags : W<u8>,  // Status
    SP: W<u8>,  // Stack pointer
    PC: W<u16>, // Program counter
}

fn load_word(memory: &mut Mem, address: W<u16>) -> u16 {
    let low = memory.load(address.0) as u16;
    (memory.load((address + W(1)).0) as u16) << 8 | low
}

fn write_word(memory: &mut Mem, address: W<u16>, word: u16) {
    memory.write(address.0, (word >> 8) as u8);
    memory.write((address + W(1)).0, word as u8);
}

impl CPU {
    pub fn new() -> CPU {
        CPU {
            A : W(0),
            X : W(0),
            Y : W(0),
            Flags : W(0x24), 
            SP : W(0xff),
            PC : W(0),
        }
    }

    fn pop(&mut self, memory: &mut Mem) -> u8 {
        self.SP = self.SP + W(1);
        memory.load(STACK_PAGE | (self.SP.0 as u16))
    }

    fn push(&mut self, memory: &mut Mem, byte: u8) {
        memory.write(STACK_PAGE | (self.SP.0 as u16), byte);
        self.SP = self.SP - W(1);
    }

    fn push_word(&mut self, memory: &mut Mem, word: u16) {
        self.push(memory, (word >> 8) as u8);
        self.push(memory, word as u8);
    }

    fn pop_word(&mut self, memory: &mut Mem) -> u16 {
        let low = self.pop(memory) as u16; 
        (self.pop(memory) as u16) << 8 | low
    }

    pub fn execute(&mut self, memory: &mut Mem) {
        let mut pc = self.PC;
        let opcode = memory.load(pc.0);
        pc = pc + W(1);
        if opcode & OP_JUMP_MASK == OP_JUMP {
            /* JMP */
            let mut address = load_word(memory, pc); 
            if opcode & !OP_JUMP_MASK > 0 {
                // Indirect Jump, +2 Cycles
                address = load_word(memory, W(address));
            } 
            self.jmp(memory, address);
        } else if opcode & OP_SPECIAL_MASK == OP_SPECIAL {
            /* Special */
            if opcode == OP_JSR {
                self.jsr(memory);
            } else {
                let index = (opcode >> 5) & 0x3;
                OP_SPECIAL_TABLE[index as usize](self, memory);
            }
        } else if opcode & OP_BRANCH_MASK == OP_BRANCH {
            /* Branch */
            let mut offset = memory.load(pc.0) as i8;
            // To sign-magnitude
            if offset < 0 { 
                offset = -(offset & 0x7F);
            }
            let index = opcode >> 5;
            OP_BRANCH_TABLE[index as usize](self, memory, offset);
        } else if opcode & OP_IMPLIED_MASK == OP_IMPLIED {
            /* Implied */
            let index = ((opcode >> 4) & 0xE) + ((opcode >> 1) & 1);
            OP_IMPLIED_TABLE[index as usize](self, memory);
        } else { 
            /* Common Operations */
            let addressing = (opcode >> 2) & 0x3;
            let index = ((opcode >> 3) & 0x1C) + (opcode & 0x3);
            OP_COMMON_TABLE[index as usize](self, memory, addressing);
        } 
    }
}

// Instructions

impl CPU {

    // Special

    fn invalid(&mut self, memory: &mut Mem) -> () {

    }

    fn brk(&mut self, memory: &mut Mem) -> () {
        
    }

    fn rti(&mut self, memory: &mut Mem) -> () {
        
    }

    fn rts(&mut self, memory: &mut Mem) -> () {
        
    }

    // Jumps

    fn jmp(&mut self, memory: &mut Mem, address: u16) {
        
    }

    fn jsr(&mut self, memory: &mut Mem) {
        let pc = self.PC + W(1);
        let address = load_word(memory, pc);
        self.push_word(memory, (pc + W(2)).0);
        self.PC = W(address);
    }

    // Branches

    fn bpl (&mut self, memory: &mut Mem, offset: i8) {

    }

    fn bmi (&mut self, memory: &mut Mem, offset: i8) {

    }

    fn bvc (&mut self, memory: &mut Mem, offset: i8) {

    }

    fn bvs (&mut self, memory: &mut Mem, offset: i8) {

    }

    fn bcc (&mut self, memory: &mut Mem, offset: i8) {

    }

    fn bcs (&mut self, memory: &mut Mem, offset: i8) {

    }

    fn bne (&mut self, memory: &mut Mem, offset: i8) {

    }

    fn beq (&mut self, memory: &mut Mem, offset: i8) {

    }
    
    // Implied

    fn php (&mut self, memory: &mut Mem) {

    }

    fn asl_a (&mut self, memory: &mut Mem) {

    }

    fn clc (&mut self, memory: &mut Mem) {

    }

    fn plp (&mut self, memory: &mut Mem) {

    }

    fn rol_a (&mut self, memory: &mut Mem) {

    }

    fn sec (&mut self, memory: &mut Mem) {

    }

    fn pha (&mut self, memory: &mut Mem) {

    }

    fn lsr_a (&mut self, memory: &mut Mem) {

    }

    fn cli (&mut self, memory: &mut Mem) {

    }

    fn pla (&mut self, memory: &mut Mem) {

    }

    fn ror_a (&mut self, memory: &mut Mem) {

    }

    fn sei (&mut self, memory: &mut Mem) {

    }

    fn dey (&mut self, memory: &mut Mem) {

    }

    fn txa (&mut self, memory: &mut Mem) {

    }

    fn tya (&mut self, memory: &mut Mem) {

    }

    fn txs (&mut self, memory: &mut Mem) {

    }

    fn tay (&mut self, memory: &mut Mem) {

    }

    fn tax (&mut self, memory: &mut Mem) {

    }

    fn clv (&mut self, memory: &mut Mem) {

    }

    fn tsx (&mut self, memory: &mut Mem) {

    }

    fn iny (&mut self, memory: &mut Mem) {
        self.Y = self.Y + W(1);
        if self.Y == 0{
            set_zero!(self.Flags);
        }else{
            unset_zero!(self.Flags);
        }
        uset_negative!(self.Flags, self.Y)
    }

    fn dex (&mut self, memory: &mut Mem) {

    }

    fn cld (&mut self, memory: &mut Mem) {

    }

    fn inx (&mut self, memory: &mut Mem) {
        self.X = self.X + W(1);
        if self.X == 0{
            set_zero!(self.Flags);
        }else{
            unset_zero!(self.Flags);
        }
        uset_negative!(self.Flags, self.X)
    }

    fn nop (&mut self, memory: &mut Mem) {

    }

    fn sed (&mut self, memory: &mut Mem) {

    }

    // Common

    fn invalid_c(&mut self, memory: &mut Mem, addressing: u8) -> () {

    }

    fn ora (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn asl (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn bit (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn and (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn rol (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn eor (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn lsr (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn adc (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn ror (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn sty (&mut self, memory: &mut Mem, addressing: u8) {
        /*let add_res : W<u8> = self.X + W(1);
        
        if (self.X ^ add_res) & (W(1) ^ add_res) & W(0x80) > W(1) {
            set_overflow!(self.Flags);
        }else{
            unset_overflow!(self.Flags);
        }*/
    }

    fn sta (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn stx (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn ldy (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn lda (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn ldx (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn cpy (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn cmp (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn dec (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn cpx (&mut self, memory: &mut Mem, addressing: u8) {

    }

    fn sbc (&mut self, memory: &mut Mem, addressing: u8) {

    }
   
    fn inc (&mut self, memory: &mut Mem, addressing: u8) {

    }
}

impl fmt::Display for CPU {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{{ A: {}, X: {}, Y: {}, P: {}, SP: {}, PC: {} }}",
               self.A.0 , self.X.0 , self.Y.0 , self.Flags.0 , self.SP.0 , self.PC.0)
    }
}


