use std::collections::{HashMap, HashSet};

use runtime::SegOfs;

use crate::{
    db::DB,
    function::{Function, XRef},
};

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
    let mut all_xrefs = HashSet::new();
    for block in func.blocks.iter_mut() {
        let ip = block.ip;
        for instr in block.instrs.iter_mut() {
            use iced_x86::FlowControl::*;
            let xref = match instr.iced.flow_control() {
                Next | Return | Interrupt => continue,
                Call | IndirectCall | IndirectBranch | UnconditionalBranch | ConditionalBranch => {
                    use iced_x86::OpKind::*;
                    match instr.iced.op0_kind() {
                        NearBranch16 | FarBranch16 => {
                            let ip = match instr.iced.op0_kind() {
                                NearBranch16 => ip.with_ofs(instr.iced.near_branch16()),
                                FarBranch16 => {
                                    (instr.iced.far_branch_selector(), instr.iced.far_branch16())
                                        .into()
                                }
                                _ => unreachable!(),
                            };
                            match names.get(&ip) {
                                Some(name) => XRef::Name(name.clone()),
                                None => XRef::Addr(ip),
                            }
                        }
                        Memory => XRef::Name("mem?".into()),
                        Register => XRef::Name("reg?".into()),
                        d => todo!("unhandled jmp {d:?}"),
                    }
                }
                XbeginXabortXend | Exception => todo!(),
            };
            instr.jmp = Some(xref.clone());
            all_xrefs.insert(xref);
        }
    }
    let mut xrefs = all_xrefs.into_iter().collect::<Vec<_>>();
    if !xrefs.is_empty() {
        xrefs.sort();
        func.xrefs = Some(xrefs);
    }
}
