use crate::db::DB;
use runtime::SegOfs;

pub fn prompt(db: &DB, addr: SegOfs) -> String {
    let mut functions = vec![];
    for func in db.functions.values() {
        for block in func.blocks.iter() {
            for instr in block.instrs.iter() {
                if let Some(Ok(mem_ref)) = instr.memory {
                    if mem_ref == addr {
                        functions.push(func.ip);
                    }
                }
            }
        }
    }

    format!(
        indoc::indoc! {"
            analyze usage of memory at address {addr} to figure out its name, description, and type.
            use tool read_memory() only to read data memory, not program code.
            to read program code, use get_function() to get the assembly.
            to start with, the address is referenced by these functions: {functions}
        "},
        addr = addr,
        functions = functions
            .iter()
            .map(|ip| ip.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )
}
