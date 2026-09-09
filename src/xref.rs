use std::collections::{HashMap, HashSet};

use runtime::SegOfs;

use crate::{db::DB, function::Function};

pub fn update_xrefs(db: &mut DB) {
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
    Name(String),
    Addr(SegOfs),
    Block(usize, Option<String>),
}

impl std::str::FromStr for XRef {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let xref = if let Some(label) = s.strip_prefix("'") {
            let label = if let Some((_, block)) = label.split_once('@') {
                block
            } else {
                label
            };
            XRef::Block(
                usize::from_str(label).map_err(|err| format!("xref {:?}: {}", s, err))?,
                None,
            )
        } else if s.contains(':') {
            XRef::Addr(SegOfs::parse(s)?)
        } else {
            XRef::Name(s.to_owned())
        };
        Ok(xref)
    }
}

impl std::fmt::Display for XRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XRef::Addr(segofs) => write!(f, "{segofs}"),
            XRef::Name(name) => f.write_str(name),
            XRef::Block(idx, None) => write!(f, "'{idx}"),
            XRef::Block(idx, Some(label)) => write!(f, "'{label}@{idx}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xref_forms_round_trip() {
        let cases = [
            ("main", XRef::Name("main".into())),
            ("1234:abcd", XRef::Addr(SegOfs::new(0x1234, 0xabcd))),
            ("'0", XRef::Block(0, None)),
            ("'42", XRef::Block(42, None)),
        ];

        for (text, expected) in cases {
            let parsed: XRef = text.parse().unwrap();
            assert!(parsed == expected, "incorrect parsing of {text:?}");
            assert_eq!(parsed.to_string(), text);
            assert_eq!(expected.to_string(), text);
            assert!(expected.to_string().parse::<XRef>().unwrap() == expected);
        }
    }

    #[test]
    fn labeled_xref_round_trip_preserves_block_index() {
        let xref = XRef::Block(42, Some("loop".into()));
        assert_eq!(xref.to_string(), "'loop@42");

        // Labels are display annotations; parsing retains only the block index.
        let parsed: XRef = xref.to_string().parse().unwrap();
        assert!(parsed == XRef::Block(42, None));
        assert_eq!(parsed.to_string(), "'42");
        assert!(parsed.to_string().parse::<XRef>().unwrap() == parsed);
    }
}
