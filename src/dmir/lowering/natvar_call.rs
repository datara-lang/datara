//! M10 NatVar + N1 List.new call intercepts (split out of expr_call.rs for
//! the 60 KB release size gate). These run BEFORE the generic builtin
//! dispatch — the generic path emits raw calls with wrong arity/ABI.

use super::Lowering;
use crate::ast::Expr;
use crate::dmir::ir::{BasicBlockId, Inst, ValueId};

impl<'a> Lowering<'a> {
    pub(crate) fn lower_natvar_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        cur_block: &mut BasicBlockId,
    ) -> Option<ValueId> {
        // M10 NatVar: fiber runtime builtins. Must run BEFORE the generic
        // builtin dispatch (which would emit a raw 2-arg call by name).
        if let Expr::Identifier(fn_name, _) = callee {
            match fn_name.as_str() {
                "fiber_spawn" | "datara_rt_fiber_spawn" if args.len() >= 2 => {
                    let fn_addr = match &args[0] {
                        Expr::Identifier(f, _) => {
                            let v = self.next_val();
                            self.get_block_mut(*cur_block)
                                .instructions
                                .push(Inst::GetFuncAddr {
                                    dest: v,
                                    func_name: f.clone(),
                                });
                            Some(v)
                        }
                        _ => None,
                    };
                    let fn_addr = fn_addr?;
                    let arg = self.lower_expr(&args[1], cur_block).unwrap_or_else(|| {
                        let z = self.next_val();
                        self.get_block_mut(*cur_block)
                            .instructions
                            .push(Inst::ConstInt { dest: z, value: 0 });
                        z
                    });
                    let stack_kb = if args.len() >= 3 {
                        self.lower_expr(&args[2], cur_block).unwrap_or_else(|| {
                            let z = self.next_val();
                            self.get_block_mut(*cur_block)
                                .instructions
                                .push(Inst::ConstInt { dest: z, value: 8 });
                            z
                        })
                    } else {
                        let z = self.next_val();
                        self.get_block_mut(*cur_block)
                            .instructions
                            .push(Inst::ConstInt { dest: z, value: 8 });
                        z
                    };
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_fiber_spawn".into(),
                            args: vec![fn_addr, arg, stack_kb],
                            ty: "Int".into(),
                        });
                    return Some(dest);
                }
                "fiber_yield" | "datara_rt_fiber_yield" if args.is_empty() => {
                    let dummy = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest: dummy,
                            func: "datara_rt_fiber_yield".into(),
                            args: vec![],
                            ty: "Unit".into(),
                        });
                    return Some(dummy);
                }
                "fiber_sleep_ms" | "datara_rt_fiber_sleep_ms" if args.len() == 1 => {
                    let ms = self.lower_expr(&args[0], cur_block).unwrap_or(ValueId(0));
                    let dummy = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest: dummy,
                            func: "datara_rt_fiber_sleep_ms".into(),
                            args: vec![ms],
                            ty: "Unit".into(),
                        });
                    return Some(dummy);
                }
                "fiber_run_until_done" | "datara_rt_fiber_run_until_done" if args.is_empty() => {
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_fiber_run_until_done".into(),
                            args: vec![],
                            ty: "Int".into(),
                        });
                    return Some(dest);
                }
                "fiber_join" | "datara_rt_fiber_join" if args.len() == 1 => {
                    let h = self.lower_expr(&args[0], cur_block).unwrap_or(ValueId(0));
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_fiber_join".into(),
                            args: vec![h],
                            ty: "Int".into(),
                        });
                    return Some(dest);
                }
                "fiber_chan_create" | "datara_rt_fiber_chan_create" if args.is_empty() => {
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_fiber_chan_create".into(),
                            args: vec![],
                            ty: "Int".into(),
                        });
                    return Some(dest);
                }
                "fiber_chan_send" | "datara_rt_fiber_chan_send" if args.len() == 2 => {
                    let c = self.lower_expr(&args[0], cur_block).unwrap_or(ValueId(0));
                    let v = self.lower_expr(&args[1], cur_block).unwrap_or(ValueId(0));
                    let dummy = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest: dummy,
                            func: "datara_rt_fiber_chan_send".into(),
                            args: vec![c, v],
                            ty: "Unit".into(),
                        });
                    return Some(dummy);
                }
                "fiber_mt_start" | "datara_rt_fiber_mt_start" if args.len() == 1 => {
                    let n = self.lower_expr(&args[0], cur_block).unwrap_or(ValueId(0));
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_fiber_mt_start".into(),
                            args: vec![n],
                            ty: "Int".into(),
                        });
                    return Some(dest);
                }
                "fiber_mt_spawn" | "datara_rt_fiber_mt_spawn" if args.len() == 2 => {
                    let fn_addr = match &args[0] {
                        Expr::Identifier(f, _) => {
                            let v = self.next_val();
                            self.get_block_mut(*cur_block)
                                .instructions
                                .push(Inst::GetFuncAddr {
                                    dest: v,
                                    func_name: f.clone(),
                                });
                            Some(v)
                        }
                        _ => None,
                    };
                    let fn_addr = fn_addr?;
                    let arg = self.lower_expr(&args[1], cur_block).unwrap_or(ValueId(0));
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_fiber_mt_spawn".into(),
                            args: vec![fn_addr, arg],
                            ty: "Int".into(),
                        });
                    return Some(dest);
                }
                "fiber_mt_join" | "datara_rt_fiber_mt_join" if args.len() == 1 => {
                    let h = self.lower_expr(&args[0], cur_block).unwrap_or(ValueId(0));
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_fiber_mt_join".into(),
                            args: vec![h],
                            ty: "Int".into(),
                        });
                    return Some(dest);
                }
                "fiber_mt_wait_idle" | "datara_rt_fiber_mt_wait_idle" if args.is_empty() => {
                    let dummy = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest: dummy,
                            func: "datara_rt_fiber_mt_wait_idle".into(),
                            args: vec![],
                            ty: "Unit".into(),
                        });
                    return Some(dummy);
                }
                "fiber_mt_done_count" | "datara_rt_fiber_mt_done_count" if args.is_empty() => {
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_fiber_mt_done_count".into(),
                            args: vec![],
                            ty: "Int".into(),
                        });
                    return Some(dest);
                }
                "fiber_mt_spawned" | "datara_rt_fiber_mt_spawned" if args.is_empty() => {
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_fiber_mt_spawned".into(),
                            args: vec![],
                            ty: "Int".into(),
                        });
                    return Some(dest);
                }
                "fiber_chan_recv" | "datara_rt_fiber_chan_recv" if args.len() == 1 => {
                    let c = self.lower_expr(&args[0], cur_block).unwrap_or(ValueId(0));
                    let dest = self.next_val();
                    self.get_block_mut(*cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest,
                            func: "datara_rt_fiber_chan_recv".into(),
                            args: vec![c],
                            ty: "Int".into(),
                        });
                    return Some(dest);
                }
                _ => {}
            }
        }
        // N1: List.new(n) — MemberAccess callee (object = class name "List");
        // NOT inside the Identifier branch above.
        if let Expr::MemberAccess { object, member, .. } = callee
            && matches!(&**object, Expr::Identifier(n, _) if n == "List")
            && member == "new"
            && args.len() == 1
        {
            let n_val = self.lower_expr(&args[0], cur_block).unwrap_or(ValueId(0));
            let zero = self.next_val();
            self.get_block_mut(*cur_block)
                .instructions
                .push(Inst::ConstInt {
                    dest: zero,
                    value: 0,
                });
            let dest = self.next_val();
            self.get_block_mut(*cur_block)
                .instructions
                .push(Inst::Call {
                    dest,
                    func: "datara_rt_list_create_repeat".into(),
                    args: vec![zero, n_val],
                    ty: "List".into(),
                });
            return Some(dest);
        }
        None
    }
}
