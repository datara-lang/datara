//!
//! Statement-flow helpers for the type checker, split out of stmt.rs to
//! keep that file under the release size gate (60 KB,
//! test_release_file_size_limits): return-exhaustiveness, break/return
//! flow proofs, for-arm binding and SIMD loop-bound checks.
use crate::ast::*;
use crate::diagnostics::{DiagnosticEngine, ErrorCode, SourceSpan};
use crate::types::{DataraType, LiteralValue, MutabilityKind, TypeChecker};

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_return_exhaustiveness(
        &self,
        fn_name: &str,
        body: &Stmt,
        expected: &DataraType,
        span: &SourceSpan,
        diag: &mut DiagnosticEngine,
    ) {
        if *expected == DataraType::Unit || matches!(expected, DataraType::TypeParam(_)) {
            return;
        }

        if !Self::stmt_guarantees_return(body) {
            diag.error_with_help(
                ErrorCode::TypeMissingReturn,
                format!(
                    "Function '{}' has declared return type '{}' but not all code paths return a value",
                    fn_name, expected
                ),
                Some(span.clone()),
                Some(format!("add an explicit 'return <expr>;' at the end of function '{}'", fn_name)),
            );
        }
    }

    pub(crate) fn stmt_guarantees_return(stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Return(Some(_), _) => true,
            Stmt::Block(stmts, _) => {
                if stmts.is_empty() {
                    return false;
                }
                for (i, s) in stmts.iter().enumerate() {
                    if Self::stmt_guarantees_return(s) {
                        return true;
                    }
                    if i == stmts.len() - 1 && matches!(s, Stmt::Expr(..)) {
                        return true;
                    }
                }
                false
            }
            Stmt::If {
                then_branch,
                else_branch: Some(else_branch),
                ..
            } => {
                Self::stmt_guarantees_return(then_branch)
                    && Self::stmt_guarantees_return(else_branch)
            }
            Stmt::Expr(Expr::Match { arms, .. }, _) => !arms.is_empty(),
            Stmt::Expr(Expr::Decide { else_arm, .. }, _) => else_arm.is_some(),
            Stmt::Expr(..) => true,
            // An unconditional loop only guarantees the enclosing function
            // never returns normally when its body cannot `break` out of it.
            Stmt::Loop { body, .. } => !Self::stmt_can_break(body),
            Stmt::While {
                condition: Expr::Literal(LiteralValue::Bool(true), _),
                body,
                ..
            } => !Self::stmt_can_break(body),
            Stmt::With { body, .. } | Stmt::Unsafe { body, .. } => {
                Self::stmt_guarantees_return(body)
            }
            _ => false,
        }
    }

    /// True when the statement subtree contains a `break` that escapes the
    /// *enclosing* loop. Breaks inside nested `while`/`for`/`loop` bodies
    /// target their own loop and do not count; `parallel` bodies are
    /// rejected by the checker before this matters.
    pub(crate) fn stmt_can_break(stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Break(_) => true,
            Stmt::Block(stmts, _) => stmts.iter().any(Self::stmt_can_break),
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                Self::stmt_can_break(then_branch)
                    || else_branch
                        .as_ref()
                        .is_some_and(|s| Self::stmt_can_break(s))
            }
            Stmt::With { body, .. } | Stmt::Unsafe { body, .. } | Stmt::Parallel(body, _) => {
                Self::stmt_can_break(body)
            }
            // A nested loop body is its own break scope.
            Stmt::While { .. } | Stmt::For { .. } | Stmt::Loop { .. } => false,
            _ => false,
        }
    }

    /// Shared body of `for` / `parallel for` checking: infer the element
    /// type, bind the loop variable, and check the loop body.
    pub(crate) fn check_for_arm(
        &mut self,
        var_name: &str,
        iterable: &Expr,
        body: &Stmt,
        diag: &mut DiagnosticEngine,
    ) -> DataraType {
        let iter_type = self.check_expr(iterable, diag);
        // Parametric collections carry their element types into the
        // loop variable: `for x in List(t)` binds `x: t`. Maps have no
        // key/value iterator protocol (the runtime lowers iteration to
        // the list protocol), so their elements stay dynamic `Val`.
        let elem_type = match &iter_type {
            DataraType::List(elem) => (**elem).clone(),
            DataraType::Map(..) => DataraType::Val,
            DataraType::GenericInstance { name, args }
                if (name == "List" || name == "Array") && !args.is_empty() =>
            {
                args[0].clone()
            }
            DataraType::Range { base, .. } => (**base).clone(),
            DataraType::Class(c) if c == "Range" => DataraType::Int,
            DataraType::String => DataraType::Char,
            DataraType::Class(c) if c == "List" || c == "Array" => {
                // Erased collections: fall back to the recorded element
                // type, else the dynamic type — never a silent Int.
                if let Expr::Identifier(n, _) = iterable {
                    self.var_element_types
                        .get(n)
                        .cloned()
                        .unwrap_or(DataraType::Val)
                } else {
                    self.last_list_element.clone().unwrap_or(DataraType::Val)
                }
            }
            _ => DataraType::Val,
        };
        if let Some(ref fn_name) = self.current_fn_name {
            self.fn_symbol_types
                .insert((fn_name.clone(), var_name.to_string()), elem_type.clone());
        }
        let prev = self.symbol_types.insert(var_name.to_string(), elem_type);
        let prev_mut = self
            .symbol_mutability
            .insert(var_name.to_string(), MutabilityKind::Immutable);
        self.check_stmt(body, diag);
        if let Some(p) = prev {
            self.symbol_types.insert(var_name.to_string(), p);
        } else {
            self.symbol_types.remove(var_name);
        }
        if let Some(m) = prev_mut {
            self.symbol_mutability.insert(var_name.to_string(), m);
        } else {
            self.symbol_mutability.remove(var_name);
        }
        DataraType::Unit
    }

    pub(crate) fn check_simd_loop_bounds(&self, stmt: &Stmt, diag: &mut DiagnosticEngine) {
        match stmt {
            Stmt::For {
                iterable,
                span,
                body,
                ..
            } => {
                if let Expr::Range { start, end, .. } = iterable {
                    let is_proven = match &**end {
                        Expr::Literal(LiteralValue::Int(n), _) => {
                            let start_val = match &**start {
                                Expr::Literal(LiteralValue::Int(s), _) => *s,
                                _ => 0,
                            };
                            (n - start_val) % 4 == 0
                        }
                        Expr::Identifier(var_name, _) => self
                            .active_requires
                            .iter()
                            .any(|req| expr_proves_mod_4(req, var_name)),
                        _ => false,
                    };
                    if !is_proven {
                        diag.error(
                            ErrorCode::RangeViolation,
                            "unproven loop bound for SIMD chunk — use require len % 4 == 0 or handle tail explicitly".to_string(),
                            Some(span.clone()),
                        );
                    }
                }
                self.check_simd_loop_bounds(body, diag);
            }
            Stmt::While {
                condition,
                span,
                body,
            } => {
                if let Expr::Binary { op, right, .. } = condition {
                    if op == "<" || op == "<=" {
                        let is_proven = match &**right {
                            Expr::Literal(LiteralValue::Int(n), _) => *n % 4 == 0,
                            Expr::Identifier(var_name, _) => self
                                .active_requires
                                .iter()
                                .any(|req| expr_proves_mod_4(req, var_name)),
                            _ => false,
                        };
                        if !is_proven {
                            diag.error(
                                ErrorCode::RangeViolation,
                                "unproven loop bound for SIMD chunk — use require len % 4 == 0 or handle tail explicitly".to_string(),
                                Some(span.clone()),
                            );
                        }
                    }
                }
                self.check_simd_loop_bounds(body, diag);
            }
            Stmt::Block(stmts, _) => {
                for s in stmts {
                    self.check_simd_loop_bounds(s, diag);
                }
            }
            _ => {}
        }
    }

    /// v1.4.5 W1 parity with `let`: is the returned expression an INT
    /// LITERAL that fits the declared narrow integer return type?
    /// Literal-only coercibility — a runtime `Int` value never narrows
    /// implicitly (Gate 7 preserved). Negative literals arrive as
    /// `Unary('-', lit)` and are peeled the same way as in `Stmt::Let`.
    pub(crate) fn return_is_narrow_literal(expr: Option<&Expr>, expected: &DataraType) -> bool {
        let Some(e) = expr else {
            return false;
        };
        let v = match e {
            Expr::Literal(LiteralValue::Int(v), _) => Some(*v),
            Expr::Unary { op, expr, .. } if op == "-" => match expr.as_ref() {
                Expr::Literal(LiteralValue::Int(v), _) => Some(-v),
                _ => None,
            },
            _ => None,
        };
        match v {
            Some(v) => {
                DataraType::is_compatible_narrow_literal(expected, &DataraType::Int, true)
                    && DataraType::narrow_literal_fits(expected, v)
            }
            None => false,
        }
    }
}

fn expr_proves_mod_4(expr: &Expr, var_name: &str) -> bool {
    match expr {
        Expr::Binary {
            op, left, right, ..
        } if op == "==" => {
            let check_mod = |m_expr: &Expr, zero_expr: &Expr| -> bool {
                if let Expr::Literal(LiteralValue::Int(0), _) = zero_expr {
                    if let Expr::Binary {
                        op, left, right, ..
                    } = m_expr
                    {
                        if op == "%" {
                            if let (
                                Expr::Identifier(id, _),
                                Expr::Literal(LiteralValue::Int(4), _),
                            ) = (&**left, &**right)
                            {
                                return id == var_name;
                            }
                        }
                    }
                }
                false
            };
            check_mod(left, right) || check_mod(right, left)
        }
        _ => false,
    }
}
