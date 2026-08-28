use runtime::SegOfs;
mod ai;

const DOSBOX_SEG: u16 = 0x813;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ai::call().await?;
    Ok(())
}

fn load() {
    let mut mem = Vec::<u8>::new();
    let psp_segment = DOSBOX_SEG;
    let load_addr = SegOfs::new(psp_segment + 0x10, 0);

    let dos = {
        let args = std::env::args().collect::<Vec<String>>();
        let path = &args[1];
        let buf = std::fs::read(path).unwrap();
        println!("loading {path}");
        let dos = exe::DOS::parse(&buf).unwrap();
        {
            let data = &buf[dos.image_offset()..];
            mem.resize(load_addr.abs() as usize + data.len(), 0);
            mem[load_addr.abs() as usize..].copy_from_slice(data);
        }
        dos.apply_relocations(load_addr.seg, &mut mem[load_addr.abs() as usize..]);

        dos
    };

    let cs = load_addr.seg + dos.header.initial_cs;
    gather_block(&mem, SegOfs::new(cs, dos.header.entry_point));
}

fn gather_block(mem: &[u8], block_addr: SegOfs) -> Vec<iced_x86::Instruction> {
    let mut block = Vec::new();
    let decoder = iced_x86::Decoder::with_ip(
        16,
        &mem[block_addr.abs() as usize..],
        block_addr.ofs as u64,
        iced_x86::DecoderOptions::NONE,
    );
    for instr in decoder.into_iter() {
        println!(
            "{addr} {code}",
            addr = block_addr.with_ofs(instr.ip16()),
            code = instr
        );
        block.push(instr);

        use iced_x86::FlowControl::*;
        match instr.flow_control() {
            Next | Call | Interrupt => {}
            _ => break,
        }
    }
    block
}
