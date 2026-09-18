use crate::ast::*;
use crate::dmir::ir::*;
use crate::types::DataraType;

use super::Lowering;

impl<'a> Lowering<'a> {
    pub fn lower_out_stmt(
        &mut self,
        e: &Expr,
        mut cur_block: BasicBlockId,
    ) -> (BasicBlockId, Option<ValueId>) {
        if let Expr::InterpolatedString {
            parts,
            expressions,
            ..
        } = e
        {
            // Format Fusion: zero-allocation streaming direct to stdout buffer
            for (idx, part) in parts.iter().enumerate() {
                if !part.is_empty() {
                    let str_val = self.next_val();
                    self.get_block_mut(cur_block)
                        .instructions
                        .push(Inst::ConstStr {
                            dest: str_val,
                            value: part.clone(),
                        });
                    let call_dest = self.next_val();
                    self.get_block_mut(cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest: call_dest,
                            func: "datara_rt_print_str".into(),
                            args: vec![str_val],
                            ty: "Unit".into(),
                        });
                }
                if idx < expressions.len() {
                    let sub_expr = &expressions[idx];
                    let call_str = if let Some(DataraType::Class(cls)) =
                        self.infer_expr_datara_type(sub_expr)
                    {
                        self.resolver.classes.get(&cls).and_then(|c| {
                            let m = if c.methods.contains_key("to_string") {
                                "to_string"
                            } else if c.methods.contains_key("to_str") {
                                "to_str"
                            } else {
                                ""
                            };
                            if !m.is_empty() {
                                Some(Expr::Call {
                                    callee: Box::new(Expr::MemberAccess {
                                        object: Box::new(sub_expr.clone()),
                                        member: m.to_string(),
                                        span: sub_expr.span().clone(),
                                    }),
                                    args: Vec::new(),
                                    span: sub_expr.span().clone(),
                                })
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    };
                    let target_sub = call_str.as_ref().unwrap_or(sub_expr);
                    if let Some(arg_val) = self.lower_expr(target_sub, &mut cur_block) {
                        let print_func = if self.is_expr_str(target_sub) {
                            "datara_rt_print_str"
                        } else if self.is_expr_float(target_sub) {
                            "datara_rt_print_float"
                        } else if self.is_expr_bool(target_sub) {
                            "datara_rt_print_bool"
                        } else {
                            "datara_rt_print_int"
                        };
                        let call_dest = self.next_val();
                        self.get_block_mut(cur_block)
                            .instructions
                            .push(Inst::Call {
                                dest: call_dest,
                                func: print_func.into(),
                                args: vec![arg_val],
                                ty: "Unit".into(),
                            });
                    }
                }
            }
            let nl_dest = self.next_val();
            self.get_block_mut(cur_block)
                .instructions
                .push(Inst::Call {
                    dest: nl_dest,
                    func: "datara_rt_print_newline".into(),
                    args: vec![],
                    ty: "Unit".into(),
                });
            return (cur_block, None);
        }

        let call_str = if let Some(DataraType::Class(cls)) = self.infer_expr_datara_type(e) {
            self.resolver.classes.get(&cls).and_then(|c| {
                let m = if c.methods.contains_key("to_string") {
                    "to_string"
                } else if c.methods.contains_key("to_str") {
                    "to_str"
                } else {
                    ""
                };
                if !m.is_empty() {
                    Some(Expr::Call {
                        callee: Box::new(Expr::MemberAccess {
                            object: Box::new(e.clone()),
                            member: m.to_string(),
                            span: e.span().clone(),
                        }),
                        args: Vec::new(),
                        span: e.span().clone(),
                    })
                } else {
                    None
                }
            })
        } else {
            None
        };
        let target_e = call_str.as_ref().unwrap_or(e);
        if let Some(val) = self.lower_expr(target_e, &mut cur_block) {
            self.get_block_mut(cur_block)
                .instructions
                .push(Inst::Out { value: val });
        }
        (cur_block, None)
    }

    pub fn lower_err_stmt(
        &mut self,
        e: &Expr,
        mut cur_block: BasicBlockId,
    ) -> (BasicBlockId, Option<ValueId>) {
        if let Expr::InterpolatedString {
            parts,
            expressions,
            ..
        } = e
        {
            // Format Fusion for err: zero-allocation streaming direct to stderr
            for (idx, part) in parts.iter().enumerate() {
                if !part.is_empty() {
                    let str_val = self.next_val();
                    self.get_block_mut(cur_block)
                        .instructions
                        .push(Inst::ConstStr {
                            dest: str_val,
                            value: part.clone(),
                        });
                    let call_dest = self.next_val();
                    self.get_block_mut(cur_block)
                        .instructions
                        .push(Inst::Call {
                            dest: call_dest,
                            func: "datara_rt_err_print_str".into(),
                            args: vec![str_val],
                            ty: "Unit".into(),
                        });
                }
                if idx < expressions.len() {
                    let sub_expr = &expressions[idx];
                    let call_str = if let Some(DataraType::Class(cls)) =
                        self.infer_expr_datara_type(sub_expr)
                    {
                        self.resolver.classes.get(&cls).and_then(|c| {
                            let m = if c.methods.contains_key("to_string") {
                                "to_string"
                            } else if c.methods.contains_key("to_str") {
                                "to_str"
                            } else {
                                ""
                            };
                            if !m.is_empty() {
                                Some(Expr::Call {
                                    callee: Box::new(Expr::MemberAccess {
                                        object: Box::new(sub_expr.clone()),
                                        member: m.to_string(),
                                        span: sub_expr.span().clone(),
                                    }),
                                    args: Vec::new(),
                                    span: sub_expr.span().clone(),
                                })
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    };
                    let target_sub = call_str.as_ref().unwrap_or(sub_expr);
                    if let Some(arg_val) = self.lower_expr(target_sub, &mut cur_block) {
                        let print_func = if self.is_expr_str(target_sub) {
                            "datara_rt_err_print_str"
                        } else if self.is_expr_float(target_sub) {
                            "datara_rt_err_print_float"
                        } else if self.is_expr_bool(target_sub) {
                            "datara_rt_err_print_bool"
                        } else {
                            "datara_rt_err_print_int"
                        };
                        let call_dest = self.next_val();
                        self.get_block_mut(cur_block)
                            .instructions
                            .push(Inst::Call {
                                dest: call_dest,
                                func: print_func.into(),
                                args: vec![arg_val],
                                ty: "Unit".into(),
                            });
                    }
                }
            }
            let nl_dest = self.next_val();
            self.get_block_mut(cur_block)
                .instructions
                .push(Inst::Call {
                    dest: nl_dest,
                    func: "datara_rt_err_print_newline".into(),
                    args: vec![],
                    ty: "Unit".into(),
                });
            return (cur_block, None);
        }

        let call_str = if let Some(DataraType::Class(cls)) = self.infer_expr_datara_type(e) {
            self.resolver.classes.get(&cls).and_then(|c| {
                let m = if c.methods.contains_key("to_string") {
                    "to_string"
                } else if c.methods.contains_key("to_str") {
                    "to_str"
                } else {
                    ""
                };
                if !m.is_empty() {
                    Some(Expr::Call {
                        callee: Box::new(Expr::MemberAccess {
                            object: Box::new(e.clone()),
                            member: m.to_string(),
                            span: e.span().clone(),
                        }),
                        args: Vec::new(),
                        span: e.span().clone(),
                    })
                } else {
                    None
                }
            })
        } else {
            None
        };
        let target_e = call_str.as_ref().unwrap_or(e);
        if let Some(val) = self.lower_expr(target_e, &mut cur_block) {
            self.get_block_mut(cur_block)
                .instructions
                .push(Inst::Err { value: val });
        }
        (cur_block, None)
    }
}
