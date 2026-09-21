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
            specs,
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
                    self.get_block_mut(cur_block).instructions.push(Inst::Call {
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
                    // v1.4.5 W4: a format specifier converts the value to a
                    // string FIRST, then streams it like any other string
                    // part. The fused path must honor the spec or out
                    // fmt"{x:.2}" would silently print full precision.
                    let spec = specs.get(idx).map(|s| s.as_str()).unwrap_or("");
                    if !spec.is_empty()
                        && let Some((conv_fn, consts, spec_ty)) =
                            self.fmt_spec_converter(target_sub, spec)
                        && let Some(arg_val) = self.lower_expr(target_sub, &mut cur_block)
                    {
                        let mut call_args = vec![arg_val];
                        for c in consts {
                            let c_dest = self.next_val();
                            self.get_block_mut(cur_block)
                                .instructions
                                .push(Inst::ConstInt {
                                    dest: c_dest,
                                    value: c,
                                });
                            call_args.push(c_dest);
                        }
                        let conv_dest = self.next_val();
                        self.get_block_mut(cur_block).instructions.push(Inst::Call {
                            dest: conv_dest,
                            func: conv_fn.into(),
                            args: call_args,
                            ty: spec_ty.into(),
                        });
                        let call_dest = self.next_val();
                        self.get_block_mut(cur_block).instructions.push(Inst::Call {
                            dest: call_dest,
                            func: "datara_rt_print_str".into(),
                            args: vec![conv_dest],
                            ty: "Unit".into(),
                        });
                        continue;
                    }
                    if let Some(arg_val) = self.lower_expr(target_sub, &mut cur_block) {
                        // v1.4.5 W3: Dec64 must be checked BEFORE the float /
                        // int fallbacks — it rides in an I64 slot, so printing
                        // it via print_int would emit the raw mantissa.
                        let print_func = if self.is_expr_str(target_sub) {
                            "datara_rt_print_str"
                        } else if self.is_expr_dec64(target_sub) {
                            "datara_rt_print_dec64"
                        } else if self.is_expr_float(target_sub) {
                            "datara_rt_print_float"
                        } else if self.is_expr_bool(target_sub) {
                            "datara_rt_print_bool"
                        } else {
                            "datara_rt_print_int"
                        };
                        let call_dest = self.next_val();
                        self.get_block_mut(cur_block).instructions.push(Inst::Call {
                            dest: call_dest,
                            func: print_func.into(),
                            args: vec![arg_val],
                            ty: "Unit".into(),
                        });
                    }
                }
            }
            let nl_dest = self.next_val();
            self.get_block_mut(cur_block).instructions.push(Inst::Call {
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
            let out_ty = self
                .infer_expr_datara_type(target_e)
                .map(|t| t.to_string())
                .unwrap_or_else(|| "Int".into());
            self.get_block_mut(cur_block).instructions.push(Inst::Out {
                value: val,
                ty: out_ty,
            });
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
            specs,
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
                    self.get_block_mut(cur_block).instructions.push(Inst::Call {
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
                    // v1.4.5 W4: the fused err path honors format specs too:
                    // convert to the formatted string, then err-print it.
                    let spec = specs.get(idx).map(|s| s.as_str()).unwrap_or("");
                    if !spec.is_empty()
                        && let Some((conv_fn, consts, spec_ty)) =
                            self.fmt_spec_converter(target_sub, spec)
                        && let Some(arg_val) = self.lower_expr(target_sub, &mut cur_block)
                    {
                        let mut call_args = vec![arg_val];
                        for c in consts {
                            let c_dest = self.next_val();
                            self.get_block_mut(cur_block)
                                .instructions
                                .push(Inst::ConstInt {
                                    dest: c_dest,
                                    value: c,
                                });
                            call_args.push(c_dest);
                        }
                        let conv_dest = self.next_val();
                        self.get_block_mut(cur_block).instructions.push(Inst::Call {
                            dest: conv_dest,
                            func: conv_fn.into(),
                            args: call_args,
                            ty: spec_ty.into(),
                        });
                        let call_dest = self.next_val();
                        self.get_block_mut(cur_block).instructions.push(Inst::Call {
                            dest: call_dest,
                            func: "datara_rt_err_print_str".into(),
                            args: vec![conv_dest],
                            ty: "Unit".into(),
                        });
                        continue;
                    }
                    if let Some(arg_val) = self.lower_expr(target_sub, &mut cur_block) {
                        // v1.4.5 W3: no dedicated Dec64 err-printer exists, so a
                        // Dec64 value is formatted through dec_to_str (exact
                        // fixed-point text) and emitted on stderr as a string.
                        let print_func = if self.is_expr_str(target_sub) {
                            "datara_rt_err_print_str"
                        } else if self.is_expr_dec64(target_sub) {
                            "datara_rt_err_print_dec64_str"
                        } else if self.is_expr_float(target_sub) {
                            "datara_rt_err_print_float"
                        } else if self.is_expr_bool(target_sub) {
                            "datara_rt_err_print_bool"
                        } else {
                            "datara_rt_err_print_int"
                        };
                        let call_dest = self.next_val();
                        self.get_block_mut(cur_block).instructions.push(Inst::Call {
                            dest: call_dest,
                            func: print_func.into(),
                            args: vec![arg_val],
                            ty: "Unit".into(),
                        });
                    }
                }
            }
            let nl_dest = self.next_val();
            self.get_block_mut(cur_block).instructions.push(Inst::Call {
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
