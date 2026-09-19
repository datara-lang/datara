//! Compile-Time Function Execution (CTFE) & Comptime Engine (v1.4.5)
//!
//! Provides Turing-complete compile-time evaluation (Zig-style `comptime fn`):
//! - Evaluates pure functions, arithmetic, strings, List/Map literals, if/while at compile-time.
//! - Strict recursion depth limit (64) and step limit (1,000,000) mapped to `E-CT-001`.
//! - Strict prohibition of I/O, runtime builtins, and unsafe blocks mapped to `E-CT-002`.
//! - Verification of compile-time constant arguments mapped to `E-CT-003`.
//! - Lowering hook emitting DMIR constant literals (`ConstInt`, `ConstFloat`, `ConstStr`, etc.).

use std::collections::HashMap;
use std::sync::Arc;

use crate::ast::{Expr, FunctionDecl, LiteralValue, SourceSpan, Stmt};
use crate::diagnostics::{DiagnosticEngine, ErrorCode};

/// Maximum recursion depth allowed during comptime evaluation.
pub const MAX_RECURSION_DEPTH: usize = 64;

/// Maximum instruction steps allowed during comptime evaluation.
pub const MAX_COMPTIME_STEPS: usize = 1_000_000;

/// Values manipulated during compile-time evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum ComptimeValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    List(Vec<ComptimeValue>),
    Map(HashMap<String, ComptimeValue>),
    Unit,
    None,
}

impl ComptimeValue {
    pub fn as_int(&self) -> Option<i64> {
        match self {
            ComptimeValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            ComptimeValue::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ComptimeValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            ComptimeValue::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn to_literal_value(&self) -> Option<LiteralValue> {
        match self {
            ComptimeValue::Int(i) => Some(LiteralValue::Int(*i)),
            ComptimeValue::Float(f) => Some(LiteralValue::Float(*f)),
            ComptimeValue::Bool(b) => Some(LiteralValue::Bool(*b)),
            ComptimeValue::Str(s) => Some(LiteralValue::String(s.clone())),
            ComptimeValue::Unit | ComptimeValue::None => Some(LiteralValue::None),
            _ => None,
        }
    }

    pub fn from_literal_value(lit: &LiteralValue) -> Self {
        match lit {
            LiteralValue::Int(i) => ComptimeValue::Int(*i),
            LiteralValue::Float(f) => ComptimeValue::Float(*f),
            LiteralValue::Bool(b) => ComptimeValue::Bool(*b),
            LiteralValue::String(s) => ComptimeValue::Str(s.clone()),
            LiteralValue::Char(c) => ComptimeValue::Int(*c as i64),
            LiteralValue::None => ComptimeValue::None,
        }
    }
}

/// Errors raised during compile-time evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum ComptimeError {
    /// E-CT-001: Recursion depth limit exceeded.
    RecursionLimitExceeded(SourceSpan),
    /// E-CT-001: Step limit exceeded.
    StepLimitExceeded(SourceSpan),
    /// E-CT-002: Forbidden effect (I/O, unsafe, runtime builtins).
    ForbiddenEffect(String, SourceSpan),
    /// E-CT-003: Arguments to comptime function must be compile-time constants.
    NonConstArg(String, SourceSpan),
    /// Division by zero at compile time.
    DivisionByZero(SourceSpan),
    /// Undefined variable.
    UndefinedVariable(String, SourceSpan),
    /// Type mismatch in expression.
    TypeMismatch(String, SourceSpan),
    /// Contract or assertion failure.
    ContractViolation(String, SourceSpan),
    /// Unsupported expression or construct.
    UnsupportedExpression(String, SourceSpan),
    /// Internal signal for return statement.
    Return(ComptimeValue),
}

impl ComptimeError {
    /// Emits this comptime error into the diagnostic engine.
    pub fn emit(&self, diag: &mut DiagnosticEngine) {
        match self {
            ComptimeError::RecursionLimitExceeded(span) => {
                diag.error(
                    ErrorCode::ComptimeRecursionLimit,
                    format!(
                        "Recursion depth limit ({}) exceeded in comptime execution",
                        MAX_RECURSION_DEPTH
                    ),
                    Some(span.clone()),
                );
            }
            ComptimeError::StepLimitExceeded(span) => {
                diag.error(
                    ErrorCode::ComptimeRecursionLimit,
                    format!(
                        "Step limit ({}) exceeded in comptime execution (infinite loop detected)",
                        MAX_COMPTIME_STEPS
                    ),
                    Some(span.clone()),
                );
            }
            ComptimeError::ForbiddenEffect(msg, span) => {
                diag.error(
                    ErrorCode::ComptimeForbiddenEffect,
                    format!(
                        "Effect '{}' not allowed in comptime execution (I/O, runtime builtins, and unsafe are forbidden)",
                        msg
                    ),
                    Some(span.clone()),
                );
            }
            ComptimeError::NonConstArg(msg, span) => {
                diag.error(
                    ErrorCode::ComptimeNonConstArg,
                    format!(
                        "Arguments to comptime function must be compile-time constants: '{}'",
                        msg
                    ),
                    Some(span.clone()),
                );
            }
            ComptimeError::DivisionByZero(span) => {
                diag.error(
                    ErrorCode::RangeViolation,
                    "Division by zero in compile-time evaluation".to_string(),
                    Some(span.clone()),
                );
            }
            ComptimeError::UndefinedVariable(name, span) => {
                diag.error(
                    ErrorCode::ResolveUndefinedSymbol,
                    format!("Undefined variable '{}' in comptime evaluation", name),
                    Some(span.clone()),
                );
            }
            ComptimeError::TypeMismatch(msg, span) => {
                diag.error(
                    ErrorCode::TypeMismatch,
                    format!("Type mismatch in comptime evaluation: {}", msg),
                    Some(span.clone()),
                );
            }
            ComptimeError::ContractViolation(msg, span) => {
                diag.error(
                    ErrorCode::ContractViolation,
                    format!("Comptime contract violation: {}", msg),
                    Some(span.clone()),
                );
            }
            ComptimeError::UnsupportedExpression(msg, span) => {
                diag.error(
                    ErrorCode::SyntaxUnexpectedToken,
                    format!("Unsupported construct in comptime evaluation: {}", msg),
                    Some(span.clone()),
                );
            }
            ComptimeError::Return(_) => {}
        }
    }
}

/// Scope environment storing variable bindings during CTFE.
#[derive(Debug, Clone, Default)]
pub struct ComptimeScope {
    vars: HashMap<String, ComptimeValue>,
    parent: Option<Box<ComptimeScope>>,
}

impl ComptimeScope {
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
            parent: None,
        }
    }

    pub fn with_parent(parent: ComptimeScope) -> Self {
        Self {
            vars: HashMap::new(),
            parent: Some(Box::new(parent)),
        }
    }

    pub fn get(&self, name: &str) -> Option<ComptimeValue> {
        if let Some(val) = self.vars.get(name) {
            Some(val.clone())
        } else if let Some(parent) = &self.parent {
            parent.get(name)
        } else {
            None
        }
    }

    pub fn set(&mut self, name: String, val: ComptimeValue) {
        self.vars.insert(name, val);
    }

    pub fn update(&mut self, name: &str, val: ComptimeValue) -> bool {
        if self.vars.contains_key(name) {
            self.vars.insert(name.to_string(), val);
            true
        } else if let Some(parent) = &mut self.parent {
            parent.update(name, val)
        } else {
            false
        }
    }
}

/// Compile-Time Function Execution engine.
#[derive(Debug, Clone)]
pub struct ComptimeEvaluator {
    functions: HashMap<String, Arc<FunctionDecl>>,
    step_count: usize,
    call_depth: usize,
}

impl Default for ComptimeEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl ComptimeEvaluator {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            step_count: 0,
            call_depth: 0,
        }
    }

    pub fn with_functions(functions: HashMap<String, FunctionDecl>) -> Self {
        let f_map = functions.into_iter().map(|(k, v)| (k, Arc::new(v))).collect();
        Self {
            functions: f_map,
            step_count: 0,
            call_depth: 0,
        }
    }

    pub fn register_function(&mut self, f: FunctionDecl) {
        self.functions.insert(f.name.clone(), Arc::new(f));
    }

    fn step(&mut self, span: &SourceSpan) -> Result<(), ComptimeError> {
        self.step_count += 1;
        if self.step_count > MAX_COMPTIME_STEPS {
            return Err(ComptimeError::StepLimitExceeded(span.clone()));
        }
        Ok(())
    }

    /// Evaluates an expression at compile time.
    pub fn eval_expr(
        &mut self,
        expr: &Expr,
        scope: &mut ComptimeScope,
    ) -> Result<ComptimeValue, ComptimeError> {
        self.step(expr.span())?;
        match expr {
            Expr::Literal(lit, _) => Ok(ComptimeValue::from_literal_value(lit)),
            Expr::Identifier(name, span) => scope
                .get(name)
                .ok_or_else(|| ComptimeError::UndefinedVariable(name.clone(), span.clone())),
            Expr::Binary {
                op,
                left,
                right,
                span,
            } => {
                let l_val = self.eval_expr(left, scope)?;
                let r_val = self.eval_expr(right, scope)?;
                self.eval_binary_op(op, l_val, r_val, span)
            }
            Expr::Unary { op, expr, span } => {
                let val = self.eval_expr(expr, scope)?;
                self.eval_unary_op(op, val, span)
            }
            Expr::Wrapping(inner, _) | Expr::Saturating(inner, _) => self.eval_expr(inner, scope),
            Expr::Comptime { expr: inner, .. } => self.eval_expr(inner, scope),
            Expr::ListLiteral(items, _) => {
                let mut vals = Vec::with_capacity(items.len());
                for it in items {
                    vals.push(self.eval_expr(it, scope)?);
                }
                Ok(ComptimeValue::List(vals))
            }
            Expr::MapLiteral(entries, _) => {
                let mut map = HashMap::with_capacity(entries.len());
                for (k, v) in entries {
                    let k_val = self.eval_expr(k, scope)?;
                    let v_val = self.eval_expr(v, scope)?;
                    let k_str = match k_val {
                        ComptimeValue::Str(s) => s,
                        ComptimeValue::Int(i) => i.to_string(),
                        _ => format!("{:?}", k_val),
                    };
                    map.insert(k_str, v_val);
                }
                Ok(ComptimeValue::Map(map))
            }
            Expr::IndexAccess {
                object,
                index,
                span,
            } => self.eval_index_access(object, index, span, scope),
            Expr::MemberAccess {
                object,
                member,
                span,
            } => {
                let obj_val = self.eval_expr(object, scope)?;
                match (obj_val, member.as_str()) {
                    (ComptimeValue::List(list), "len") => Ok(ComptimeValue::Int(list.len() as i64)),
                    (ComptimeValue::Str(s), "len") => Ok(ComptimeValue::Int(s.len() as i64)),
                    (ComptimeValue::Map(m), "len") => Ok(ComptimeValue::Int(m.len() as i64)),
                    _ => Err(ComptimeError::TypeMismatch(
                        format!("Unknown member '{}' on comptime value", member),
                        span.clone(),
                    )),
                }
            }
            Expr::Call { callee, args, span } => self.eval_call(callee, args, span, scope),
            Expr::Block(stmts, trailing_expr, _) => self.eval_block(stmts, trailing_expr.as_deref(), scope),
            Expr::Decide {
                arms,
                else_arm,
                span: _,
            } => self.eval_decide(arms, else_arm.as_deref(), scope),
            _ => Err(ComptimeError::UnsupportedExpression(
                format!("{:?}", expr),
                expr.span().clone(),
            )),
        }
    }

    #[inline(never)]
    fn eval_index_access(
        &mut self,
        object: &Expr,
        index: &Expr,
        span: &SourceSpan,
        scope: &mut ComptimeScope,
    ) -> Result<ComptimeValue, ComptimeError> {
        let col_val = self.eval_expr(object, scope)?;
        let idx_val = self.eval_expr(index, scope)?;
        match (col_val, idx_val) {
            (ComptimeValue::List(list), ComptimeValue::Int(idx)) => {
                let i = if idx < 0 {
                    list.len() as i64 + idx
                } else {
                    idx
                };
                if i < 0 || (i as usize) >= list.len() {
                    return Err(ComptimeError::ContractViolation(
                        format!("Index {} out of bounds (len: {})", idx, list.len()),
                        span.clone(),
                    ));
                }
                Ok(list[i as usize].clone())
            }
            (ComptimeValue::Map(map), ComptimeValue::Str(key)) => {
                Ok(map.get(&key).cloned().unwrap_or(ComptimeValue::None))
            }
            _ => Err(ComptimeError::TypeMismatch(
                "Cannot index non-collection in comptime".to_string(),
                span.clone(),
            )),
        }
    }

    #[inline(never)]
    fn eval_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        span: &SourceSpan,
        scope: &mut ComptimeScope,
    ) -> Result<ComptimeValue, ComptimeError> {
        match callee {
            Expr::Identifier(fn_name, _) => {
                let mut evaluated_args = Vec::with_capacity(args.len());
                for a in args {
                    evaluated_args.push(self.eval_expr(a, scope)?);
                }
                self.call_fn(fn_name, evaluated_args, span)
            }
            Expr::MemberAccess { object, member, .. } => {
                let mut evaluated_args = Vec::with_capacity(args.len());
                for a in args {
                    evaluated_args.push(self.eval_expr(a, scope)?);
                }

                if let Expr::Identifier(var_name, _) = &**object {
                    if let Some(mut current_val) = scope.get(var_name) {
                        if member == "push" && evaluated_args.len() == 1 {
                            if let ComptimeValue::List(list) = &mut current_val {
                                list.push(evaluated_args.remove(0));
                                scope.update(var_name, current_val);
                                return Ok(ComptimeValue::Unit);
                            }
                        } else if member == "insert" && evaluated_args.len() == 2 {
                            if let ComptimeValue::Map(map) = &mut current_val {
                                let k_str = match &evaluated_args[0] {
                                    ComptimeValue::Str(s) => s.clone(),
                                    ComptimeValue::Int(i) => i.to_string(),
                                    other => format!("{:?}", other),
                                };
                                map.insert(k_str, evaluated_args.remove(1));
                                scope.update(var_name, current_val);
                                return Ok(ComptimeValue::Unit);
                            }
                        }
                    }
                }

                let obj_val = self.eval_expr(object, scope)?;
                match (obj_val, member.as_str()) {
                    (ComptimeValue::List(list), "len") => Ok(ComptimeValue::Int(list.len() as i64)),
                    (ComptimeValue::Str(s), "len") => Ok(ComptimeValue::Int(s.len() as i64)),
                    (ComptimeValue::Map(m), "len") => Ok(ComptimeValue::Int(m.len() as i64)),
                    (ComptimeValue::Map(m), "get") if !evaluated_args.is_empty() => {
                        let k = match &evaluated_args[0] {
                            ComptimeValue::Str(s) => s.as_str(),
                            _ => "",
                        };
                        Ok(m.get(k).cloned().unwrap_or(ComptimeValue::None))
                    }
                    _ => Err(ComptimeError::TypeMismatch(
                        format!("Unknown method '{}' on comptime object", member),
                        span.clone(),
                    )),
                }
            }
            _ => Err(ComptimeError::UnsupportedExpression(
                "Indirect comptime function calls are unsupported".to_string(),
                span.clone(),
            )),
        }
    }

    #[inline(never)]
    fn eval_block(
        &mut self,
        stmts: &[Stmt],
        trailing_expr: Option<&Expr>,
        scope: &mut ComptimeScope,
    ) -> Result<ComptimeValue, ComptimeError> {
        let mut inner_scope = ComptimeScope::with_parent(scope.clone());
        for s in stmts {
            match self.eval_stmt(s, &mut inner_scope) {
                Ok(()) => {}
                Err(ComptimeError::Return(val)) => return Ok(val),
                Err(e) => return Err(e),
            }
        }
        for (k, v) in inner_scope.vars {
            scope.update(&k, v);
        }
        if let Some(t_expr) = trailing_expr {
            self.eval_expr(t_expr, scope)
        } else {
            Ok(ComptimeValue::Unit)
        }
    }

    #[inline(never)]
    fn eval_decide(
        &mut self,
        arms: &[crate::ast::DecideArm],
        else_arm: Option<&Expr>,
        scope: &mut ComptimeScope,
    ) -> Result<ComptimeValue, ComptimeError> {
        for arm in arms {
            let cond_val = self.eval_expr(&arm.condition, scope)?;
            match cond_val {
                ComptimeValue::Bool(true) => return self.eval_expr(&arm.body, scope),
                ComptimeValue::Bool(false) => continue,
                _ => {
                    return Err(ComptimeError::TypeMismatch(
                        "Decide condition must evaluate to Bool in comptime".to_string(),
                        arm.condition.span().clone(),
                    ));
                }
            }
        }
        if let Some(else_expr) = else_arm {
            self.eval_expr(else_expr, scope)
        } else {
            Ok(ComptimeValue::Unit)
        }
    }

    /// Evaluates a statement at compile time.
    pub fn eval_stmt(
        &mut self,
        stmt: &Stmt,
        scope: &mut ComptimeScope,
    ) -> Result<(), ComptimeError> {
        self.step(stmt.span())?;
        match stmt {
            Stmt::Let { name, init, .. }
            | Stmt::Mut { name, init, .. }
            | Stmt::Const { name, init, .. } => {
                let val = self.eval_expr(init, scope)?;
                scope.set(name.clone(), val);
                Ok(())
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => self.eval_assign(target, value, span, scope),
            Stmt::Expr(e, _) => {
                let _ = self.eval_expr(e, scope)?;
                Ok(())
            }
            Stmt::Block(stmts, _) => {
                for s in stmts {
                    self.eval_stmt(s, scope)?;
                }
                Ok(())
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let cond_val = self.eval_expr(condition, scope)?;
                match cond_val {
                    ComptimeValue::Bool(true) => self.eval_stmt(then_branch, scope),
                    ComptimeValue::Bool(false) => {
                        if let Some(eb) = else_branch {
                            self.eval_stmt(eb, scope)
                        } else {
                            Ok(())
                        }
                    }
                    _ => Err(ComptimeError::TypeMismatch(
                        "If condition must be boolean in comptime".to_string(),
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
                    self.step(span)?;
                    let cond_val = self.eval_expr(condition, scope)?;
                    match cond_val {
                        ComptimeValue::Bool(true) => {
                            self.eval_stmt(body, scope)?;
                        }
                        ComptimeValue::Bool(false) => break,
                        _ => {
                            return Err(ComptimeError::TypeMismatch(
                                "While condition must be boolean in comptime".to_string(),
                                span.clone(),
                            ));
                        }
                    }
                }
                Ok(())
            }
            Stmt::Return(expr_opt, _) => {
                let val = if let Some(e) = expr_opt {
                    self.eval_expr(e, scope)?
                } else {
                    ComptimeValue::Unit
                };
                Err(ComptimeError::Return(val))
            }
            Stmt::Unsafe { span, .. } => Err(ComptimeError::ForbiddenEffect(
                "unsafe".to_string(),
                span.clone(),
            )),
            Stmt::Out(_, span) | Stmt::Err(_, span) => Err(ComptimeError::ForbiddenEffect(
                "I/O".to_string(),
                span.clone(),
            )),
            _ => Ok(()),
        }
    }

    #[inline(never)]
    fn eval_assign(
        &mut self,
        target: &Expr,
        value: &Expr,
        span: &SourceSpan,
        scope: &mut ComptimeScope,
    ) -> Result<(), ComptimeError> {
        let val = self.eval_expr(value, scope)?;
        match target {
            Expr::Identifier(name, _) => {
                if !scope.update(name, val.clone()) {
                    scope.set(name.clone(), val);
                }
                Ok(())
            }
            Expr::IndexAccess {
                object,
                index,
                span: idx_span,
            } => {
                if let Expr::Identifier(name, _) = &**object {
                    let idx_val = self.eval_expr(index, scope)?;
                    if let Some(mut col_val) = scope.get(name) {
                        if let (ComptimeValue::List(list), ComptimeValue::Int(idx)) =
                            (&mut col_val, idx_val)
                        {
                            let i = if idx < 0 {
                                list.len() as i64 + idx
                            } else {
                                idx
                            };
                            if i < 0 || (i as usize) >= list.len() {
                                return Err(ComptimeError::ContractViolation(
                                    format!(
                                        "Index {} out of bounds (len: {})",
                                        idx,
                                        list.len()
                                    ),
                                    idx_span.clone(),
                                ));
                            }
                            list[i as usize] = val;
                            scope.update(name, col_val);
                            return Ok(());
                        }
                    }
                }
                Err(ComptimeError::UnsupportedExpression(
                    "Complex indexing assignment in comptime".to_string(),
                    span.clone(),
                ))
            }
            _ => Err(ComptimeError::UnsupportedExpression(
                format!("Unsupported assignment target at {:?}", span),
                span.clone(),
            )),
        }
    }

    /// Invokes a function at compile-time.
    pub fn call_fn(
        &mut self,
        name: &str,
        args: Vec<ComptimeValue>,
        span: &SourceSpan,
    ) -> Result<ComptimeValue, ComptimeError> {
        // Forbidden runtime builtins and I/O
        if matches!(
            name,
            "println"
                | "print"
                | "eprintln"
                | "eprint"
                | "read_file"
                | "write_file"
                | "exec"
                | "spawn"
                | "panic"
        ) {
            return Err(ComptimeError::ForbiddenEffect(name.to_string(), span.clone()));
        }

        // Builtin functions
        if name == "int_to_str" && args.len() == 1 {
            if let Some(i) = args[0].as_int() {
                return Ok(ComptimeValue::Str(i.to_string()));
            }
        }
        if name == "len" && args.len() == 1 {
            match &args[0] {
                ComptimeValue::List(l) => return Ok(ComptimeValue::Int(l.len() as i64)),
                ComptimeValue::Str(s) => return Ok(ComptimeValue::Int(s.len() as i64)),
                ComptimeValue::Map(m) => return Ok(ComptimeValue::Int(m.len() as i64)),
                _ => {}
            }
        }

        let func = self
            .functions
            .get(name)
            .cloned()
            .ok_or_else(|| ComptimeError::UndefinedVariable(name.to_string(), span.clone()))?;

        // Recursion depth gate (E-CT-001)
        self.call_depth += 1;
        if self.call_depth > MAX_RECURSION_DEPTH {
            self.call_depth -= 1;
            return Err(ComptimeError::RecursionLimitExceeded(span.clone()));
        }

        let mut fn_scope = ComptimeScope::new();
        for (i, p) in func.params.iter().enumerate() {
            let arg_val = args.get(i).cloned().unwrap_or(ComptimeValue::None);
            fn_scope.set(p.name.clone(), arg_val);
        }

        let result = match self.eval_stmt(&func.body, &mut fn_scope) {
            Ok(()) => Ok(ComptimeValue::Unit),
            Err(ComptimeError::Return(val)) => Ok(val),
            Err(e) => Err(e),
        };

        self.call_depth -= 1;
        result
    }

    fn eval_binary_op(
        &self,
        op: &str,
        left: ComptimeValue,
        right: ComptimeValue,
        span: &SourceSpan,
    ) -> Result<ComptimeValue, ComptimeError> {
        match (left, right) {
            (ComptimeValue::Int(a), ComptimeValue::Int(b)) => match op {
                "+" => Ok(ComptimeValue::Int(a.wrapping_add(b))),
                "-" => Ok(ComptimeValue::Int(a.wrapping_sub(b))),
                "*" => Ok(ComptimeValue::Int(a.wrapping_mul(b))),
                "/" => {
                    if b == 0 {
                        Err(ComptimeError::DivisionByZero(span.clone()))
                    } else {
                        Ok(ComptimeValue::Int(a.wrapping_div(b)))
                    }
                }
                "%" => {
                    if b == 0 {
                        Err(ComptimeError::DivisionByZero(span.clone()))
                    } else {
                        Ok(ComptimeValue::Int(a.wrapping_rem(b)))
                    }
                }
                "==" => Ok(ComptimeValue::Bool(a == b)),
                "!=" => Ok(ComptimeValue::Bool(a != b)),
                "<" => Ok(ComptimeValue::Bool(a < b)),
                "<=" => Ok(ComptimeValue::Bool(a <= b)),
                ">" => Ok(ComptimeValue::Bool(a > b)),
                ">=" => Ok(ComptimeValue::Bool(a >= b)),
                "&" => Ok(ComptimeValue::Int(a & b)),
                "|" => Ok(ComptimeValue::Int(a | b)),
                "^" => Ok(ComptimeValue::Int(a ^ b)),
                "<<" => Ok(ComptimeValue::Int(a << (b & 63))),
                ">>" => Ok(ComptimeValue::Int(a >> (b & 63))),
                _ => Err(ComptimeError::UnsupportedExpression(
                    format!("Unknown int operator: {}", op),
                    span.clone(),
                )),
            },
            (ComptimeValue::Float(a), ComptimeValue::Float(b)) => match op {
                "+" => Ok(ComptimeValue::Float(a + b)),
                "-" => Ok(ComptimeValue::Float(a - b)),
                "*" => Ok(ComptimeValue::Float(a * b)),
                "/" => Ok(ComptimeValue::Float(a / b)),
                "==" => Ok(ComptimeValue::Bool((a - b).abs() < f64::EPSILON)),
                "!=" => Ok(ComptimeValue::Bool((a - b).abs() >= f64::EPSILON)),
                "<" => Ok(ComptimeValue::Bool(a < b)),
                "<=" => Ok(ComptimeValue::Bool(a <= b)),
                ">" => Ok(ComptimeValue::Bool(a > b)),
                ">=" => Ok(ComptimeValue::Bool(a >= b)),
                _ => Err(ComptimeError::UnsupportedExpression(
                    format!("Unknown float operator: {}", op),
                    span.clone(),
                )),
            },
            (ComptimeValue::Bool(a), ComptimeValue::Bool(b)) => match op {
                "&&" => Ok(ComptimeValue::Bool(a && b)),
                "||" => Ok(ComptimeValue::Bool(a || b)),
                "==" => Ok(ComptimeValue::Bool(a == b)),
                "!=" => Ok(ComptimeValue::Bool(a != b)),
                _ => Err(ComptimeError::UnsupportedExpression(
                    format!("Unknown bool operator: {}", op),
                    span.clone(),
                )),
            },
            (ComptimeValue::Str(a), ComptimeValue::Str(b)) if op == "+" => {
                Ok(ComptimeValue::Str(format!("{}{}", a, b)))
            }
            (ComptimeValue::Str(a), ComptimeValue::Int(b)) if op == "+" => {
                Ok(ComptimeValue::Str(format!("{}{}", a, b)))
            }
            (ComptimeValue::Int(a), ComptimeValue::Str(b)) if op == "+" => {
                Ok(ComptimeValue::Str(format!("{}{}", a, b)))
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
        val: ComptimeValue,
        span: &SourceSpan,
    ) -> Result<ComptimeValue, ComptimeError> {
        match (op, val) {
            ("-", ComptimeValue::Int(i)) => Ok(ComptimeValue::Int(-i)),
            ("-", ComptimeValue::Float(f)) => Ok(ComptimeValue::Float(-f)),
            ("!", ComptimeValue::Bool(b)) => Ok(ComptimeValue::Bool(!b)),
            ("~", ComptimeValue::Int(i)) => Ok(ComptimeValue::Int(!i)),
            _ => Err(ComptimeError::TypeMismatch(
                format!("Cannot apply unary '{}' to operand", op),
                span.clone(),
            )),
        }
    }
}
