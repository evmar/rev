use std::collections::{HashMap, HashSet};

use runtime::SegOfs;

use crate::{db::DB, function::Function};

pub fn update_all_xrefs(db: &mut DB) {
    let names = db
        .functions
        .values()
        .filter_map(|func| Some((func.ip, func.name.as_ref()?.clone())))
        .collect::<HashMap<_, _>>();
    for func in db.functions.values_mut() {
        update_xrefs(func, |ip| {
            let name = names.get(&ip).cloned();
            XRef::External(name, ip)
        });
    }
}

pub fn update_xrefs(func: &mut Function, xref: impl Fn(SegOfs) -> XRef) {
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
                Some(idx) => XRef::Block(block_to_label[*idx].clone(), *idx),
                None => xref(ip),
            }) else {
                continue;
            };
            instr.jmp = Some(xref.clone());

            match &xref {
                XRef::External(_, _) => {
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
                Memory => Some(XRef::External(Some("mem?".into()), Default::default())),
                Register => Some(XRef::External(Some("reg?".into()), Default::default())),
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
