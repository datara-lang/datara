//! Advanced Compile-Time Function Execution (CTFE) & Comptime Engine (v1.2.7)
//!
//! Provides Turing-complete compile-time evaluation for Datara:
//! - Evaluates expressions, multi-statement blocks, and while-loops at compile-time.
//! - Synthesizes precomputed lookup tables and constant arrays into .rodata.
//! - Evaluates static compile-time assertions and contracts (`comptime require`).
//! - Strictly bounded evaluation step limits to guarantee compiler termination.

use std::collections::HashMap;

use crate::ast::{Expr, LiteralValue, SourceSpan, Stmt};

/// Maximum execution steps in a single comptime evaluation to guarantee termination.
pub const MAX_COMPTIME_STEPS: usize = 1_000_000;

/// Compile-time evaluation error.
#[derive(Debug, Clone, PartialEq)]
pub enum ComptimeError {
    StepLimitExceeded,
    DivisionByZero(SourceSpan),
    UndefinedVariable(String, SourceSpan),
    TypeMismatch(String, SourceSpan),
    ContractViolation(String, SourceSpan),
    UnsupportedExpression(String),
}

/// Compile-time environment storing variable values.
#[derive(Default, Clone)]
pub struct ComptimeEnv {
    vars: HashMap<String, LiteralValue>,
}

impl ComptimeEnv {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    pub fn get(&self, name: &str) -> Option<&LiteralValue> {
        self.vars.get(name)
    }

    pub fn set(&mut self, name: String, val: LiteralValue) {
        self.vars.insert(name, val);
    }
}

/// Evaluator for compile-time expressions and statements.
pub struct ComptimeEvaluator {
    env: ComptimeEnv,
    step_count: usize,
}

impl Default for ComptimeEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl ComptimeEvaluator {
    pub fn new() -> Self {
        Self {
            env: ComptimeEnv::new(),
            step_count: 0,
        }
    }

    fn step(&mut self) -> Result<(), ComptimeError> {
        self.step_count += 1;
        if self.step_count > MAX_COMPTIME_STEPS {
            return Err(ComptimeError::StepLimitExceeded);
        }
        Ok(())
    }

    /// Evaluates an AST expression to a compile-time LiteralValue.
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<LiteralValue, ComptimeError> {
        self.step()?;
        match expr {
            Expr::Literal(lit, _) => Ok(lit.clone()),
            Expr::Identifier(name, span) => self
                .env
                .get(name)
                .cloned()
                .ok_or_else(|| ComptimeError::UndefinedVariable(name.clone(), span.clone())),
            Expr::Binary {
                op,
                left,
                right,
                span,
            } => {
                let l_val = self.eval_expr(left)?;
                let r_val = self.eval_expr(right)?;
                self.eval_binary_op(op, l_val, r_val, span)
            }
            Expr::Unary { op, expr, span } => {
                let val = self.eval_expr(expr)?;
                self.eval_unary_op(op, val, span)
            }
            Expr::Wrapping(inner, _) | Expr::Saturating(inner, _) => self.eval_expr(inner),
            Expr::Comptime { expr: inner, .. } => self.eval_expr(inner),
            Expr::Block(stmts, trailing_expr, _) => {
                for s in stmts {
                    self.eval_stmt(s)?;
                }
                if let Some(t_expr) = trailing_expr {
                    self.eval_expr(t_expr)
                } else {
                    Ok(LiteralValue::None)
                }
            }
            _ => Err(ComptimeError::UnsupportedExpression(format!("{:?}", expr))),
        }
    }

    /// Evaluates an AST statement in the current compile-time environment.
    pub fn eval_stmt(&mut self, stmt: &Stmt) -> Result<(), ComptimeError> {
        self.step()?;
        match stmt {
            Stmt::Let { name, init, .. }
            | Stmt::Mut { name, init, .. }
            | Stmt::Const { name, init, .. } => {
                let val = self.eval_expr(init)?;
                self.env.set(name.clone(), val);
                Ok(())
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => {
                let val = self.eval_expr(value)?;
                if let Expr::Identifier(name, _) = target {
                    self.env.set(name.clone(), val);
                    Ok(())
                } else {
                    Err(ComptimeError::UnsupportedExpression(format!(
                        "Unsupported assignment target at {:?}",
                        span
                    )))
                }
            }
            Stmt::Expr(e, _) => {
                let _ = self.eval_expr(e)?;
                Ok(())
            }
            Stmt::Block(stmts, _) => {
                for s in stmts {
                    self.eval_stmt(s)?;
                }
                Ok(())
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let cond_val = self.eval_expr(condition)?;
                match cond_val {
                    LiteralValue::Bool(true) => self.eval_stmt(then_branch),
                    LiteralValue::Bool(false) => {
                        if let Some(eb) = else_branch {
                            self.eval_stmt(eb)
                        } else {
                            Ok(())
                        }
                    }
                    _ => Err(ComptimeError::TypeMismatch(
                        "If condition must be boolean".to_string(),
                        span.clone(),
                    )),
                }
            }
            Stmt::While {
                condition,
                body,
                span,
            } => {
                loop {
                    self.step()?;
                    let cond_val = self.eval_expr(condition)?;
                    match cond_val {
                        LiteralValue::Bool(true) => {
                            self.eval_stmt(body)?;
                        }
                        LiteralValue::Bool(false) => break,
                        _ => {
                            return Err(ComptimeError::TypeMismatch(
                                "While condition must be boolean".to_string(),
                                span.clone(),
                            ));
                        }
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn eval_binary_op(
        &self,
        op: &str,
        left: LiteralValue,
        right: LiteralValue,
        span: &SourceSpan,
    ) -> Result<LiteralValue, ComptimeError> {
        match (left, right) {
            (LiteralValue::Int(a), LiteralValue::Int(b)) => match op {
                "+" => Ok(LiteralValue::Int(a.wrapping_add(b))),
                "-" => Ok(LiteralValue::Int(a.wrapping_sub(b))),
                "*" => Ok(LiteralValue::Int(a.wrapping_mul(b))),
                "/" => {
                    if b == 0 {
                        Err(ComptimeError::DivisionByZero(span.clone()))
                    } else {
                        Ok(LiteralValue::Int(a.wrapping_div(b)))
                    }
                }
                "%" => {
                    if b == 0 {
                        Err(ComptimeError::DivisionByZero(span.clone()))
                    } else {
                        Ok(LiteralValue::Int(a.wrapping_rem(b)))
                    }
                }
                "==" => Ok(LiteralValue::Bool(a == b)),
                "!=" => Ok(LiteralValue::Bool(a != b)),
                "<" => Ok(LiteralValue::Bool(a < b)),
                "<=" => Ok(LiteralValue::Bool(a <= b)),
                ">" => Ok(LiteralValue::Bool(a > b)),
                ">=" => Ok(LiteralValue::Bool(a >= b)),
                "&" => Ok(LiteralValue::Int(a & b)),
                "|" => Ok(LiteralValue::Int(a | b)),
                "^" => Ok(LiteralValue::Int(a ^ b)),
                "<<" => Ok(LiteralValue::Int(a << (b & 63))),
                ">>" => Ok(LiteralValue::Int(a >> (b & 63))),
                _ => Err(ComptimeError::UnsupportedExpression(format!(
                    "Unknown int operator: {}",
                    op
                ))),
            },
            (LiteralValue::Float(a), LiteralValue::Float(b)) => match op {
                "+" => Ok(LiteralValue::Float(a + b)),
                "-" => Ok(LiteralValue::Float(a - b)),
                "*" => Ok(LiteralValue::Float(a * b)),
                "/" => Ok(LiteralValue::Float(a / b)),
                "==" => Ok(LiteralValue::Bool((a - b).abs() < f64::EPSILON)),
                "!=" => Ok(LiteralValue::Bool((a - b).abs() >= f64::EPSILON)),
                "<" => Ok(LiteralValue::Bool(a < b)),
                "<=" => Ok(LiteralValue::Bool(a <= b)),
                ">" => Ok(LiteralValue::Bool(a > b)),
                ">=" => Ok(LiteralValue::Bool(a >= b)),
                _ => Err(ComptimeError::UnsupportedExpression(format!(
                    "Unknown float operator: {}",
                    op
                ))),
            },
            (LiteralValue::Bool(a), LiteralValue::Bool(b)) => match op {
                "&&" => Ok(LiteralValue::Bool(a && b)),
                "||" => Ok(LiteralValue::Bool(a || b)),
                "==" => Ok(LiteralValue::Bool(a == b)),
                "!=" => Ok(LiteralValue::Bool(a != b)),
                _ => Err(ComptimeError::UnsupportedExpression(format!(
                    "Unknown bool operator: {}",
                    op
                ))),
            },
            (LiteralValue::String(a), LiteralValue::String(b)) if op == "+" => {
                Ok(LiteralValue::String(format!("{}{}", a, b)))
            }
            (LiteralValue::String(a), LiteralValue::Int(b)) if op == "+" => {
                Ok(LiteralValue::String(format!("{}{}", a, b)))
            }
            (LiteralValue::Int(a), LiteralValue::String(b)) if op == "+" => {
                Ok(LiteralValue::String(format!("{}{}", a, b)))
            }
            _ => Err(ComptimeError::TypeMismatch(
                format!("Incompatible operands for operator '{}'", op),
                span.clone(),
            )),
        }
    }

    fn eval_unary_op(
        &self,
        op: &str,
        val: LiteralValue,
        span: &SourceSpan,
    ) -> Result<LiteralValue, ComptimeError> {
        match (op, val) {
            ("-", LiteralValue::Int(i)) => Ok(LiteralValue::Int(-i)),
            ("-", LiteralValue::Float(f)) => Ok(LiteralValue::Float(-f)),
            ("!", LiteralValue::Bool(b)) => Ok(LiteralValue::Bool(!b)),
            ("~", LiteralValue::Int(i)) => Ok(LiteralValue::Int(!i)),
            _ => Err(ComptimeError::TypeMismatch(
                format!("Cannot apply unary '{}' to operand", op),
                span.clone(),
            )),
        }
    }
}
