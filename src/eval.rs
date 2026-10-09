use std::collections::{BTreeMap, HashMap};

use runtime::SegOfs;

use crate::{
    db::{Annotation, Block, DB, Instr},
    ir,
};

pub fn eval_all(db: &mut DB, fns: Vec<SegOfs>) {
    for func in db.functions.values_mut() {
        if !fns.is_empty() && !fns.contains(&func.ip) {
            continue;
        }
        println!("eval {:?}", func.name);
        for block in func.blocks.iter_mut() {
            eval_block(&mut func.annotations, block);
        }
    }
}

pub fn eval_block(annot: &mut BTreeMap<SegOfs, Annotation>, block: &mut Block) {
    let mut eval = Eval::default();
    eval.block(annot, block);
}

#[derive(Default)]
struct Eval {
    cs: u16,
    regs: HashMap<ir::Name, u16>,
}

impl Eval {
    fn block(&mut self, annot: &mut BTreeMap<SegOfs, Annotation>, block: &mut Block) {
        self.cs = block.ip.seg;
        for instr in block.instrs.iter_mut() {
            self.instr(block.ip.with_ofs(instr.ip16()), instr, annot);
        }
    }

    fn instr(&mut self, ip: SegOfs, instr: &Instr, annot: &mut BTreeMap<SegOfs, Annotation>) {
        println!("instr: {}", instr);
        if let Some(assume) = annot.get(&ip).map(|a| &a.assume) {
            for (reg, val) in assume.iter() {
                self.regs.insert(ir::Name::new(reg), *val);
            }
        }
        let stmt = match ir::Stmt::try_from(instr) {
            Ok(stmt) => stmt,
            Err(err) => {
                println!("err: {err}");
                return;
            }
        };
        if let Some(mem) = self.gather_memory(&stmt) {
            annot.entry(ip).or_default().memory = Some(mem.map_err(|err| format!("[{err}]")));
        }
        match self.eval_stmt(&stmt) {
            Ok(_) => {}
            Err(err) => println!("{}: {}", instr, err),
        }
    }

    /// (attempt to) update self.regs based on instruction's effects
    fn eval_stmt(&mut self, stmt: &ir::Stmt) -> Result<(), String> {
        println!("eval: {stmt}");
        match &stmt.kind {
            ir::StmtKind::Do(_) => {}
            ir::StmtKind::Set(dst, src) => match dst {
                ir::Expr::Reg(dst) => {
                    match self.eval_expr(src) {
                        Ok(val) => {
                            self.regs.insert(dst.clone(), val);
                        }
                        Err(err) => {
                            println!("eval {src} failed: {err}");
                            self.regs.remove(dst);
                        }
                    };
                }
                ir::Expr::Todo(_) => todo!(),
                _ => {}
            },
            ir::StmtKind::Let(_, _) => todo!(),
            ir::StmtKind::Jmp(_, _) => {}
            ir::StmtKind::Raw(_) => {}
        }
        Ok(())
    }

    /// if instruction touches memory, return its address
    fn gather_memory(&self, stmt: &ir::Stmt) -> Option<Result<SegOfs, String>> {
        // rather than looking for memory ops, look for segofs calls.
        // this catches "lea", though maybe evaluation could just as well too

        let mut mem = None;
        ir::visit_stmt_expr(stmt, &mut |expr: &ir::Expr| {
            let ir::Expr::Call(call) = expr else {
                return;
            };
            if call.func != "segofs" {
                return;
            }
            println!("gather {}", call);
            let [seg, ofs] = &call.args.as_slice() else {
                panic!();
            };
            let seg = match self.eval_expr(seg) {
                Ok(seg) => seg,
                Err(err) => {
                    mem = Some(Err(err));
                    return;
                }
            };
            let ofs = match self.eval_expr(ofs) {
                Ok(ofs) => ofs,
                Err(err) => {
                    mem = Some(Err(err));
                    return;
                }
            };
            mem = Some(Ok(SegOfs::new(seg, ofs)));
        });
        mem
    }

    fn eval_expr(&self, expr: &ir::Expr) -> Result<u16, String> {
        match expr {
            ir::Expr::Val(v) => Ok(*v as u16),
            ir::Expr::Reg(name) => self.get_reg(name).ok_or_else(|| format!("{name} unknown")),
            ir::Expr::Call(call) => match call.func.as_str() {
                "+" => {
                    let mut sum = 0;
                    for arg in call.args.iter() {
                        sum += self.eval_expr(arg)?;
                    }
                    Ok(sum)
                }
                _ => Err(format!("unhandled call {call}")),
            },
            _ => Err(format!("unhandled {expr}")),
        }
    }

    fn get_reg(&self, reg: &ir::Name) -> Option<u16> {
        if reg == "cs" {
            return Some(self.cs);
        }
        self.regs.get(&reg).cloned()
    }
}
