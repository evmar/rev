use std::collections::{HashMap, HashSet};

use runtime::SegOfs;

use crate::db::{DB, Function, Instr};

pub fn update_all_xrefs(db: &mut DB) -> bool {
    let names = db
        .functions
        .values()
        .filter_map(|func| Some((func.ip, func.name.as_ref()?.clone())))
        .collect::<HashMap<_, _>>();
    let mut callers = HashMap::<SegOfs, Vec<XRef>>::new();
    let mut changed = false;
    for func in db.functions.values_mut() {
        let xref = XRef::External(func.name.clone(), func.ip);
        if update_callees(func, &db.mem, |ip| {
            callers.entry(ip).or_default().push(xref.clone());
            let name = names.get(&ip).cloned();
            XRef::External(name, ip)
        }) {
            changed = true;
        }
    }

    for func in db.functions.values_mut() {
        let new_callers = callers.remove(&func.ip);
        if new_callers != func.callers {
            func.callers = new_callers;
            changed = true;
        }
    }
    changed
}

pub fn update_callees(
    func: &mut Function,
    mem: &[u8],
    mut xref: impl FnMut(SegOfs) -> XRef,
) -> bool {
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
            let Some(xref) = xref_from_instr(ip, &instr, mem, |ip| match ip_to_block.get(&ip) {
                Some(idx) => XRef::Block(block_to_label[*idx].clone(), *idx),
                None => xref(ip),
            }) else {
                continue;
            };

            match &xref {
                XRef::External(_, _) | XRef::Unknown(_) => {
                    all_xrefs.insert(xref.clone());
                }
                _ => {}
            }

            instr.jmp = Some(xref);
        }
    }

    let mut changed = false;
    let mut xrefs = all_xrefs.into_iter().collect::<Vec<_>>();
    if !xrefs.is_empty() {
        xrefs.sort();
        let new_callees = Some(xrefs);
        if new_callees != func.callees {
            func.callees = new_callees;
            changed = true;
        }
    }
    changed
}

fn xref_from_instr(
    ip: SegOfs,
    instr: &Instr,
    mem: &[u8],
    mut xref: impl FnMut(SegOfs) -> XRef,
) -> Option<XRef> {
    use iced_x86::FlowControl::*;
    let iced = &instr.iced;
    match iced.flow_control() {
        Next | Return | Interrupt => None,
        Call | IndirectCall | IndirectBranch | UnconditionalBranch | ConditionalBranch => {
            use iced_x86::OpKind::*;
            match iced.op0_kind() {
                NearBranch16 | FarBranch16 => {
                    let ip = match iced.op0_kind() {
                        NearBranch16 => ip.with_ofs(iced.near_branch16()),
                        FarBranch16 => (iced.far_branch_selector(), iced.far_branch16()).into(),
                        _ => unreachable!(),
                    };
                    Some(xref(ip))
                }
                // XXX: what about far calls?
                Memory => match instr.memory {
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
    External(Option<String>, SegOfs),
    Block(Option<String>, usize),
    Unknown(String),
}

impl std::str::FromStr for XRef {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let xref = if let Some(local) = s.strip_prefix("'") {
            let (name, block) = if let Some((name, block)) = local.split_once('@') {
                (Some(name.to_owned()), block)
            } else {
                (None, local)
            };
            XRef::Block(
                name,
                usize::from_str(block).map_err(|err| format!("xref {:?}: {}", s, err))?,
            )
        } else if let Some(unk) = s.strip_prefix("unknown:") {
            XRef::Unknown(unk.to_owned())
        } else {
            let (name, ip) = if let Some((name, ip)) = s.split_once('@') {
                (Some(name.to_owned()), ip)
            } else {
                (None, s)
            };
            XRef::External(name, SegOfs::parse(ip)?)
        };
        Ok(xref)
    }
}

impl std::fmt::Display for XRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XRef::External(None, ip) => write!(f, "{ip}"),
            XRef::External(Some(name), ip) => write!(f, "{name}@{ip}"),
            XRef::Block(None, idx) => write!(f, "'{idx}"),
            XRef::Block(Some(label), idx) => write!(f, "'{label}@{idx}"),
            XRef::Unknown(label) => write!(f, "unknown:{label}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xref_forms_round_trip() {
        let cases = [
            (
                "main@1234:abcd",
                XRef::External(Some("main".into()), SegOfs::new(0x1234, 0xabcd)),
            ),
            (
                "1234:abcd",
                XRef::External(None, SegOfs::new(0x1234, 0xabcd)),
            ),
            ("'foo@0", XRef::Block(Some("foo".into()), 0)),
            ("'42", XRef::Block(None, 42)),
        ];

        for (text, expected) in cases {
            let parsed: XRef = text.parse().unwrap();
            assert!(parsed == expected, "incorrect parsing of {text:?}");
            assert_eq!(parsed.to_string(), text);
            assert_eq!(expected.to_string(), text);
            assert!(expected.to_string().parse::<XRef>().unwrap() == expected);
        }
    }
}
