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
        analyze(func, |ip| match names.get(&ip) {
            Some(name) => XRef::Name(name.clone()),
            None => XRef::Addr(ip),
        });
    }

    db.write()?;
    Ok(())
}

fn analyze(func: &mut Function, xref: impl Fn(SegOfs) -> XRef) {
    let mut ip_to_block = HashMap::new();
    let mut block_to_label = Vec::new();
    for (i, block) in func.blocks.iter().enumerate() {
        let ip = block.ip;
        ip_to_block.insert(ip, i);
        block_to_label.push(block.instrs[0].label.clone());
    }

    let mut all_xrefs = HashSet::new();
    for block in func.blocks.iter_mut() {
        let ip = block.ip;
        for instr in block.instrs.iter_mut() {
            let Some(xref) = xref_from_instr(ip, &instr.iced, |ip| match ip_to_block.get(&ip) {
                Some(idx) => XRef::Block(*idx, block_to_label[*idx].clone()),
                None => xref(ip),
            }) else {
                continue;
            };
            instr.jmp = Some(xref.clone());

            match &xref {
                XRef::Name(_) | XRef::Addr(_) => {
                    all_xrefs.insert(xref);
                }
                XRef::Block(_, _) => {}
            }
        }
    }

    let mut xrefs = all_xrefs.into_iter().collect::<Vec<_>>();
    if !xrefs.is_empty() {
        xrefs.sort();
        func.xrefs = Some(xrefs);
    }
}

fn xref_from_instr(
    ip: SegOfs,
    instr: &iced_x86::Instruction,
    xref: impl Fn(SegOfs) -> XRef,
) -> Option<XRef> {
    use iced_x86::FlowControl::*;
    match instr.flow_control() {
        Next | Return | Interrupt => None,
        Call | IndirectCall | IndirectBranch | UnconditionalBranch | ConditionalBranch => {
            use iced_x86::OpKind::*;
            match instr.op0_kind() {
                NearBranch16 | FarBranch16 => {
                    let ip = match instr.op0_kind() {
                        NearBranch16 => ip.with_ofs(instr.near_branch16()),
                        FarBranch16 => (instr.far_branch_selector(), instr.far_branch16()).into(),
                        _ => unreachable!(),
                    };
                    Some(xref(ip))
                }
                Memory => Some(XRef::Name("mem?".into())),
                Register => Some(XRef::Name("reg?".into())),
                d => todo!("unhandled jmp {d:?}"),
            }
        }
        XbeginXabortXend | Exception => todo!(),
    }
}
