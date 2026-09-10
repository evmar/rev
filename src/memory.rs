use crate::{db::DB, function::Function};

pub fn gather(db: &DB) {
    for func in db.functions.values() {
        gather_func(func);
    }
}

fn gather_func(func: &Function) {
    for block in func.blocks.iter() {
        for instr in block.instrs.iter() {
            let instr = &instr.iced;
            for op in 0..instr.op_count() {
                let kind = instr.op_kind(op);
                use iced_x86::OpKind::*;
                match kind {
                    Memory => {
                        let seg = instr.memory_segment();
                        let base = instr.memory_base();
                        if base != iced_x86::Register::None {
                            // [base + ...]
                            continue;
                        }
                        let index = instr.memory_index();
                        if index != iced_x86::Register::None {
                            // [... + index*scale + ...]
                            let _scale = instr.memory_index_scale();
                            continue;
                        }
                        let disp = instr.memory_displacement32();
                        println!("{}", instr);
                        println!("{seg:?} {base:?} {disp:x}");
                    }

                    MemorySegSI | MemorySegESI | MemorySegRSI | MemorySegDI | MemorySegEDI
                    | MemorySegRDI | MemoryESDI | MemoryESEDI | MemoryESRDI => {}
                    _ => {}
                }
            }
        }
    }
}
