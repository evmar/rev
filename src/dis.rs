use crate::{
    db::DB,
    function::{Block, Function, Instr},
};
use runtime::SegOfs;
use std::collections::{BTreeMap, VecDeque};

/// disassemble
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "dis")]
pub struct Args {
    #[argh(positional, from_str_fn(SegOfs::parse))]
    addr: SegOfs,
}

pub fn run(db: &mut DB, args: Args) -> anyhow::Result<()> {
    let ip = args.addr;
    dis_func(db, ip);
    db.write()?;
    Ok(())
}

pub fn dis_func<'db>(db: &'db mut DB, ip: SegOfs) -> &'db mut Function {
    if !db.functions.contains_key(&ip) {
        let func = Function {
            ip,
            ..Default::default()
        };
        db.functions.insert(ip, func);
    }
    let func = db.functions.get_mut(&ip).unwrap();
    func.blocks = gather(&db.mem, ip);
    check_coverage(func);
    func
}

fn check_coverage(func: &Function) {
    let start = func.blocks[0].instrs[0].iced.ip16() as usize;
    let mut covered = vec![];
    for block in func.blocks.iter() {
        for instr in block.instrs.iter() {
            let end = instr.iced.ip16() as usize + instr.iced.len();
            covered.resize(end - start, false);
            covered[instr.iced.ip16() as usize - start..][..instr.iced.len()].fill(true);
        }
    }

    let count = covered.iter().filter(|&&b| b).count();
    println!(
        "{count}/{len} bytes covered",
        count = count,
        len = covered.len()
    );
}

fn gather(mem: &[u8], start: SegOfs) -> Vec<Block> {
    let mut queue = VecDeque::new();
    let mut blocks = BTreeMap::<SegOfs, Block>::new();
    queue.push_back(start);
    while let Some(ip) = queue.pop_front() {
        if let Some((&baddr, block)) = blocks.range(..=ip).last() {
            if baddr == ip {
                // already visited
                continue;
            }
            if block.contains_ip(ip) {
                blocks.remove(&baddr);
                queue.push_back(baddr);
            }
        }

        let block = gather_block(
            mem,
            ip,
            |ip| blocks.contains_key(&ip),
            |ip| queue.push_back(ip),
        );
        blocks.insert(ip, block);
    }
    blocks.into_values().collect()
}

fn gather_block(
    mem: &[u8],
    block_ip: SegOfs,
    visited: impl Fn(SegOfs) -> bool,
    mut enqueue: impl FnMut(SegOfs),
) -> Block {
    let mut instrs = Vec::new();
    let decoder = iced_x86::Decoder::with_ip(
        16,
        &mem[block_ip.abs() as usize..],
        block_ip.ofs as u64,
        iced_x86::DecoderOptions::NONE,
    );
    for instr in decoder.into_iter() {
        if visited(block_ip.with_ofs(instr.ip16())) {
            assert!(!instrs.is_empty());
            break;
        }

        // println!(
        //     "{addr} {code}",
        //     addr = block_ip.with_ofs(instr.ip16()),
        //     code = instr
        // );
        instrs.push(Instr {
            iced: instr,
            ..Default::default()
        });

        use iced_x86::FlowControl::*;
        match instr.flow_control() {
            Next => {}
            Call | IndirectCall | Interrupt => {
                // assume it returns
            }

            IndirectBranch | UnconditionalBranch | ConditionalBranch => {
                control_flow(block_ip, &instr, &mut enqueue);
                if instr.flow_control() == ConditionalBranch {
                    enqueue(block_ip.with_ofs(instr.next_ip16()));
                }
                break;
            }

            Return => break,

            XbeginXabortXend | Exception => todo!(),
        }
    }
    Block {
        ip: block_ip,
        instrs,
    }
}

fn control_flow(ip: SegOfs, instr: &iced_x86::Instruction, mut enqueue: impl FnMut(SegOfs)) {
    use iced_x86::OpKind::*;
    assert_eq!(instr.op_count(), 1);
    match instr.op0_kind() {
        NearBranch16 => enqueue(ip.with_ofs(instr.near_branch16())),
        FarBranch16 => {
            enqueue((instr.far_branch_selector(), instr.far_branch16()).into());
        }
        Memory => {}
        Register => {
            // jmp [reg]  for some register
            // log::warn!("{ip} {instr}  ; indirect via register");
        }
        d => todo!("unhandled jmp {d:?}"),
    }
}
