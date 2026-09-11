use std::collections::HashMap;

use runtime::SegOfs;

use crate::{
    db::DB,
    function::{Block, Instr},
};

pub fn eval_all(db: &mut DB) {
    for func in db.functions.values_mut() {
        println!("eval {:?}", func.name);
        for block in func.blocks.iter_mut() {
            eval_block(block);
        }
    }
}

pub fn eval_block(block: &mut Block) {
    let mut eval = Eval::default();
    eval.block(block);
}

#[derive(Default)]
struct Eval {
    cs: u16,
    regs: HashMap<iced_x86::Register, u16>,
}

impl Eval {
    fn block(&mut self, block: &mut Block) {
        self.cs = block.ip.seg;
        for instr in block.instrs.iter_mut() {
            self.instr(instr);
        }
    }

    fn instr(&mut self, instr: &mut Instr) {
        if let Some(mem) = self.gather_memory(instr) {
            match mem {
                Ok(mem) => {
                    println!("{} ; {}", instr.iced, mem);
                    instr.memory = Some(mem);
                }
                Err(err) => println!("{} ; {}", instr.iced, err),
            }
        }
        self.eval_instr(&instr.iced);
    }

    /// (attempt to) update self.regs based on instruction's effects
    fn eval_instr(&mut self, instr: &iced_x86::Instruction) {
        use iced_x86::Mnemonic::*;
        match instr.mnemonic() {
            Mov => {
                assert_eq!(instr.op_count(), 2);
                let val = self.eval_op(instr, 1);
                use iced_x86::OpKind::*;
                match instr.op0_kind() {
                    Register => {
                        let reg = instr.op0_register();
                        if reg.size() != 2 {
                            self.regs.clear();
                            return;
                        }
                        match val {
                            Some(val) => {
                                self.regs.insert(reg, val);
                                println!("{reg:?} = {val:x}");
                            }
                            None => {
                                self.regs.remove(&reg);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {
                self.regs.clear();
            }
        }
    }

    /// if instruction touches memory, return its address
    fn gather_memory(&self, instr: &Instr) -> Option<Result<SegOfs, String>> {
        use iced_x86::OpKind::*;
        for op in 0..instr.iced.op_count() {
            match instr.iced.op_kind(op) {
                Memory => {
                    let seg = instr.iced.memory_segment();
                    let Some(seg) = self.get_reg(seg) else {
                        return Some(Err(format!("unknown seg {seg:?} {:?}", self.regs)));
                    };
                    let base = instr.iced.memory_base();
                    if base != iced_x86::Register::None {
                        // [base + ...]
                        return Some(Err("base reg".into()));
                    }
                    let index = instr.iced.memory_index();
                    if index != iced_x86::Register::None {
                        // [... + index*scale + ...]
                        return Some(Err("scale".into()));
                    }
                    let disp = instr.iced.memory_displacement32() as u16;
                    return Some(Ok((seg, disp).into()));
                }
                // TODO: stosb etc
                _ => {}
            }
        }
        None
    }

    fn eval_op(&mut self, instr: &iced_x86::Instruction, op: u32) -> Option<u16> {
        use iced_x86::OpKind::*;
        match instr.op_kind(op) {
            Register => self.get_reg(instr.op_register(op)),
            Immediate16 => Some(instr.immediate16()),
            Immediate8to16 => Some(instr.immediate8to16() as u16),
            _ => None,
        }
    }

    fn get_reg(&self, reg: iced_x86::Register) -> Option<u16> {
        if reg == iced_x86::Register::CS {
            return Some(self.cs);
        }
        self.regs.get(&reg).cloned()
    }
}
