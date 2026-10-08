use std::collections::HashMap;

use runtime::SegOfs;

use crate::db::{Annotation, DB, Function, Instr};

#[derive(Default)]
pub struct XRefs {
    pub callers: HashMap<SegOfs, Vec<SegOfs>>,
    pub callees: HashMap<SegOfs, Vec<SegOfs>>,
}

impl XRefs {
    pub fn gather(db: &DB) -> Self {
        let mut xrefs = XRefs::default();
        for func in db.functions.values() {
            let callees = func.gather_callees();
            for callee in callees.iter() {
                xrefs.callers.entry(*callee).or_default().push(func.ip);
            }
            xrefs.callees.insert(func.ip, callees);
        }
        for callers in xrefs.callers.values_mut() {
            callers.sort();
        }
        xrefs
    }
}

pub fn update_xrefs(func: &mut Function, mem: &[u8]) {
    let mut ip_to_block = HashMap::new();
    for (i, block) in func.blocks.iter().enumerate() {
        let ip = block.ip;
        ip_to_block.insert(ip, i);
    }

    for block in func.blocks.iter_mut() {
        for instr in block.instrs.iter_mut() {
            let ip = block.ip.with_ofs(instr.ip16());
            let annot = func.annotations.get(&ip);
            let Some(xref) =
                xref_from_instr(ip, &instr, annot, mem, |ip| match ip_to_block.get(&ip) {
                    Some(idx) => XRef::Block(*idx),
                    None => XRef::External(ip),
                })
            else {
                continue;
            };
            func.annotations.entry(ip).or_default().jmp = Some(xref);
        }
    }
}

fn xref_from_instr(
    ip: SegOfs,
    instr: &Instr,
    annot: Option<&Annotation>,
    mem: &[u8],
    mut xref: impl FnMut(SegOfs) -> XRef,
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
                // XXX: what about far calls via memory?
                Memory => match annot.and_then(|a| a.memory.as_ref()) {
                    Some(Ok(addr)) => {
                        let mem = &mem[addr.abs() as usize..][..2];
                        let ofs = u16::from_le_bytes([mem[0], mem[1]]);
                        let ip = ip.with_ofs(ofs);
                        if ofs != 0 {
                            Some(xref(ip))
                        } else {
                            Some(XRef::Unknown(format!("mem[{ip}]=0")))
                        }
                    }
                    _ => Some(XRef::Unknown("mem".into())),
                },
                Register => Some(XRef::Unknown("reg".into())),
                d => todo!("unhandled jmp {d:?}"),
            }
        }
        XbeginXabortXend | Exception => todo!(),
    }
}

#[derive(
    serde_with::DeserializeFromStr,
    serde_with::SerializeDisplay,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
)]
pub enum XRef {
    External(SegOfs),
    Block(usize),
    Unknown(String),
}

impl std::str::FromStr for XRef {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let xref = if let Some(block) = s.strip_prefix("'") {
            XRef::Block(usize::from_str(block).map_err(|err| format!("xref {:?}: {}", s, err))?)
        } else if let Some(block) = s.strip_suffix(":") {
            let block = block
                .strip_prefix("block")
                .ok_or_else(|| format!("bad xref"))?;
            XRef::Block(usize::from_str(block).map_err(|err| format!("xref {:?}: {}", s, err))?)
        } else if let Some(unk) = s.strip_prefix("unknown:") {
            XRef::Unknown(unk.to_owned())
        } else {
            XRef::External(SegOfs::parse(s)?)
        };
        Ok(xref)
    }
}

impl std::fmt::Display for XRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XRef::External(ip) => write!(f, "{ip}"),
            XRef::Block(idx) => write!(f, "block{idx}:"),
            XRef::Unknown(label) => write!(f, "unknown:{label}"),
        }
    }
}
