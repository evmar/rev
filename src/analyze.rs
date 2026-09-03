use std::collections::HashMap;

use runtime::SegOfs;

use crate::{db::DB, function::Function};

/// wip
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "analyze")]
pub struct Args {}

pub fn run(db: &mut DB, _args: Args) -> anyhow::Result<()> {
    let names = db
        .functions
        .values()
        .filter_map(|func| Some((func.ip, func.name.as_ref()?.clone())))
        .collect::<HashMap<_, _>>();
    for func in db.functions.values_mut() {
        analyze(&names, func);
    }

    db.write()?;
    Ok(())
}

fn analyze(names: &HashMap<SegOfs, String>, func: &mut Function) {
    for block in func.blocks.iter_mut() {
        let ip = block.ip;
        for instr in block.instrs.iter_mut() {
            use iced_x86::FlowControl::*;
            match instr.iced.flow_control() {
                Next | Return | Interrupt => {}
                Call | IndirectCall | IndirectBranch | UnconditionalBranch | ConditionalBranch => {
                    use iced_x86::OpKind::*;
                    match instr.iced.op0_kind() {
                        NearBranch16 => {
                            let ip = ip.with_ofs(instr.iced.near_branch16());
                            let name = names.get(&ip).cloned().unwrap_or_else(|| format!("{}", ip));
                            instr.jmp = Some(name)
                        }
                        FarBranch16 => {
                            let ip: SegOfs =
                                (instr.iced.far_branch_selector(), instr.iced.far_branch16())
                                    .into();
                            let name = names.get(&ip).cloned().unwrap_or_else(|| format!("{}", ip));
                            instr.jmp = Some(name)
                        }
                        Memory => instr.jmp = Some(format!("mem?")),
                        Register => instr.jmp = Some(format!("reg?")),
                        d => todo!("unhandled jmp {d:?}"),
                    }
                }

                XbeginXabortXend | Exception => todo!(),
            }
        }
    }
}
