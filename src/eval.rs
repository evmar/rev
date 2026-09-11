use std::collections::HashMap;

use runtime::SegOfs;

use crate::{
    db::DB,
    function::{Block, Instr},
    ir,
};

pub fn eval_all(db: &mut DB) {
    for func in db.functions.values_mut() {
        println!("eval {:?}", func.name);
        for block in func.blocks.iter_mut() {
            eval_block(block);
        }
    }
}

pub fn eval_block(block: &mut Block) {
    let mut eval = Eval::default();
    eval.block(block);
}

#[derive(Default)]
struct Eval {
    cs: u16,
    regs: HashMap<ir::Name, u16>,
}

impl Eval {
    fn block(&mut self, block: &mut Block) {
        self.cs = block.ip.seg;
        for instr in block.instrs.iter_mut() {
            self.instr(instr);
        }
    }

    fn instr(&mut self, instr: &mut Instr) {
        println!("instr {}", instr.iced);
        let stmt = match ir::Stmt::try_from(&instr.iced) {
            Ok(stmt) => {
                println!("{stmt}");
                stmt
            }
            Err(err) => {
                println!("err: {err}");
                return;
            }
        };
        if let Some(mem) = self.gather_memory(&stmt) {
            match mem {
                Ok(mem) => {
                    println!("{} ; {}", instr.iced, mem);
                    instr.memory = Some(mem);
                }
                Err(err) => println!("{} ; {}", instr.iced, err),
            }
        }
        match self.eval_stmt(&stmt) {
            Ok(_) => {}
            Err(err) => println!("{}: {}", instr.iced, err),
        }
    }

    /// (attempt to) update self.regs based on instruction's effects
    fn eval_stmt(&mut self, stmt: &ir::Stmt) -> Result<(), String> {
        println!("eval {stmt}");
        match &stmt.kind {
            ir::StmtKind::Do(_) => {}
            ir::StmtKind::Set(dst, src) => match dst {
                ir::Expr::Reg(dst) => {
                    match self.eval_expr(src) {
                        Some(val) => {
                            self.regs.insert(dst.clone(), val);
                        }
                        None => {
                            println!("eval {src} failed");
                            self.regs.remove(dst);
                        }
                    };
                }
                ir::Expr::Todo(_) => todo!(),
                _ => {}
            },
            ir::StmtKind::Let(_, _) => todo!(),
            ir::StmtKind::Jmp(_, _) => {}
            ir::StmtKind::Raw(_) => todo!(),
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
            let [seg, ofs] = &call.args.as_slice() else {
                panic!();
            };
            let seg = match self.eval_expr(seg) {
                Some(seg) => seg,
                None => {
                    mem = Some(Err(format!("{seg} unknown")));
                    return;
                }
            };
            let ofs = match self.eval_expr(ofs) {
                Some(ofs) => ofs,
                None => {
                    mem = Some(Err(format!("{ofs} unknown")));
                    return;
                }
            };
            mem = Some(Ok(SegOfs::new(seg, ofs)));
        });
        mem
    }

    fn eval_expr(&self, expr: &ir::Expr) -> Option<u16> {
        Some(match expr {
            ir::Expr::Val(v) => *v as u16,
            ir::Expr::Reg(name) => self.get_reg(name)?,
            _ => return None,
        })
    }

    fn get_reg(&self, reg: &ir::Name) -> Option<u16> {
        if reg == "cs" {
            return Some(self.cs);
        }
        self.regs.get(&reg).cloned()
    }
}
