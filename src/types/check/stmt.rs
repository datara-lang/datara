use crate::ast::*;
use crate::diagnostics::{DiagnosticEngine, ErrorCode};
use crate::types::{DataraType, MutabilityKind, TypeChecker};

impl<'a> TypeChecker<'a> {
    pub fn record_var_type(&mut self, name: &str, ty: DataraType) {
        if let Some(ref fn_name) = self.current_fn_name {
            // DMIR/codegen only understand the erased class form
            // (`Class("List")`); keep the parametric type checker-internal.
            self.fn_symbol_types
                .insert((fn_name.clone(), name.to_string()), ty.erasure());
        }
        self.symbol_types.insert(name.to_string(), ty);
    }

    pub fn is_printable_type(&self, ty: &DataraType) -> bool {
        match ty {
            DataraType::Int
            | DataraType::UInt
            | DataraType::Int8
            | DataraType::Int16
            | DataraType::Int32
            | DataraType::UInt8
            | DataraType::UInt16
            | DataraType::UInt32
            | DataraType::UInt64
            | DataraType::Float
            | DataraType::Float32
            | DataraType::Bool
            | DataraType::Char
            | DataraType::String
            | DataraType::Dec64
            | DataraType::Val
            | DataraType::TypeParam(_) => true,
            DataraType::Range { base, .. } | DataraType::Measure { base, .. } => {
                self.is_printable_type(base)
            }
            DataraType::Class(class_name) => {
                if class_name == "Range" {
                    return true;
                }
                if self
                    .impls
                    .contains_key(&("Display".to_string(), class_name.clone()))
                {
                    return true;
                }
                if let Some(cls) = self.resolver.classes.get(class_name) {
                    if cls.methods.contains_key("to_string") || cls.methods.contains_key("to_str") {
                        return true;
                    }
                }
                false
            }
            DataraType::GenericInstance { name, .. } => {
                if name == "Range" {
                    return true;
                }
                if self
                    .impls
                    .contains_key(&("Display".to_string(), name.clone()))
                {
                    return true;
                }
                if let Some(cls) = self.resolver.classes.get(name) {
                    if cls.methods.contains_key("to_string") || cls.methods.contains_key("to_str") {
                        return true;
                    }
                }
                false
            }
            // Result-like payloads print through their two-state convention
            // (ok -> value, err -> message); `out map.get(..).is_ok()` and
            // `out Weak<T>.upgrade().is_ok()` are the common liveness probes.
            DataraType::Result(ok, _) => self.is_printable_type(ok),
            DataraType::Option(val) => self.is_printable_type(val),
            _ => false,
        }
    }

    pub fn check_stmt(&mut self, stmt: &Stmt, diag: &mut DiagnosticEngine) -> DataraType {
        match stmt {
            Stmt::Block(stmts, _) => {
                // Lexical scope: declarations inside the block must not leak
                // into sibling blocks or the enclosing scope.
                let saved_types = self.symbol_types.clone();
                let saved_mut = self.symbol_mutability.clone();
                let saved_elem = self.var_element_types.clone();
                let saved_refinements = self.var_refinements.clone();
                let saved_lengths = self.var_array_lengths.clone();
                let mut last = DataraType::Unit;
                for s in stmts {
                    last = self.check_stmt(s, diag);
                }
                self.symbol_types = saved_types;
                self.symbol_mutability = saved_mut;
                self.var_element_types = saved_elem;
                self.var_refinements = saved_refinements;
                self.var_array_lengths = saved_lengths;
                last
            }
            Stmt::Let {
                name,
                type_node,
                init,
                span,
            } => {
                // M2 machine fixes: remember where the binding was declared
                // so immutable-assignment errors can point a `val` -> `mut`
                // replacement at the declaration line.
                self.symbol_decl_spans.insert(name.clone(), span.clone());
                // v1.5.0: publish the annotation while checking the initializer
                // so generic static constructors and checked results adopt it
                // (`val m: Map<Int, Int> = Map.new()`, `val r: Outcome<Int> =
                // Outcome.err("..")`). Mirrors Stmt::Mut.
                let saved_assign_target = self.current_assign_target.take();
                if let Some(tn) = type_node {
                    self.current_assign_target = Some(self.resolve_type_node(tn, diag));
                }
                let init_type = self.check_expr(init, diag);
                self.current_assign_target = saved_assign_target;
                let final_ty = if let Some(tn) = type_node {
                    self.var_refinements.insert(name.clone(), tn.clone());
                    self.check_refinement(tn, init, span, diag);
                    let declared = self.resolve_type_node(tn, diag);
                    let is_literal_numeric = matches!(
                        init,
                        Expr::Literal(LiteralValue::Float(_), _)
                            | Expr::Literal(LiteralValue::Int(_), _)
                    );
                    let compatible = if let DataraType::Measure { base, .. } = &declared {
                        (is_literal_numeric
                            && init_type.is_compatible_with_args(base, Some(self.resolver)))
                            || init_type.is_compatible_with_refined_with_args(
                                &declared,
                                Some(self.resolver),
                            )
                    } else {
                        init_type
                            .is_compatible_with_refined_with_args(&declared, Some(self.resolver))
                    };
                    // v1.4.5 W1: an in-range INT LITERAL coerces to a narrow
                    // integer target (`let a: Int8 = 100`). The actual range
                    // proof lives in `check_range_and_measure_assignment`,
                    // which fires right below; a runtime Int value never
                    // narrows implicitly (Gate 7).
                    // Negative literals arrive as Unary('-', lit): peel so
                    // `let a: Int8 = -128` proves in-range too.
                    let literal_in_range = match init {
                        Expr::Literal(LiteralValue::Int(v), _) => {
                            DataraType::narrow_literal_fits(&declared, *v)
                        }
                        Expr::Unary { op, expr, .. } if op == "-" => match expr.as_ref() {
                            Expr::Literal(LiteralValue::Int(v), _) => {
                                DataraType::narrow_literal_fits(&declared, -v)
                            }
                            _ => false,
                        },
                        _ => false,
                    };
                    let compatible = compatible
                        || DataraType::is_compatible_narrow_literal(
                            &declared,
                            &init_type,
                            literal_in_range,
                        )
                        || DataraType::is_compatible_float_literal(
                            &declared,
                            &init_type,
                            matches!(init, Expr::Literal(LiteralValue::Float(_), _)),
                        );
                    if !compatible {
                        let help_msg = Self::suggest_type_fix(&declared, &init_type);
                        // M2 fix 4/5: a Str initializer where Int/Float is
                        // declared gets a wrap-in-str_to_int/str_to_float
                        // machine edit (source-sliced from the expression).
                        let machine_fix = self.str_parse_machine_fix(&declared, &init_type, init);
                        diag.error_with_fix(
                            ErrorCode::TypeMismatch,
                            format!(
                                "Type mismatch in variable declaration: expected '{}', got '{}'",
                                declared, init_type
                            ),
                            Some(span.clone()),
                            help_msg,
                            machine_fix,
                        );
                    }
                    self.check_range_and_measure_assignment(
                        &declared, &init_type, init, span, diag,
                    );
                    declared
                } else {
                    init_type
                };
                self.record_var_type(name, final_ty.clone());
                if let Expr::ListLiteral(elements, _) = init {
                    self.var_array_lengths.insert(name.clone(), elements.len());
                    if let Some(e) = self.last_list_element.take() {
                        self.var_element_types.insert(name.clone(), e);
                    }
                } else if let Expr::ArrayRepeatLiteral { count, .. } = init {
                    self.var_array_lengths.insert(name.clone(), *count);
                }
                self.symbol_mutability
                    .insert(name.clone(), MutabilityKind::Immutable);
                final_ty
            }
            Stmt::Const {
                name,
                type_node,
                init,
                span,
            } => {
                let init_type = self.check_expr(init, diag);
                let final_ty = if let Some(tn) = type_node {
                    self.var_refinements.insert(name.clone(), tn.clone());
                    self.check_refinement(tn, init, span, diag);
                    let declared = self.resolve_type_node(tn, diag);
                    let is_literal_numeric = matches!(
                        init,
                        Expr::Literal(LiteralValue::Float(_), _)
                            | Expr::Literal(LiteralValue::Int(_), _)
                    );
                    let compatible = if let DataraType::Measure { base, .. } = &declared {
                        (is_literal_numeric
                            && init_type.is_compatible_with_args(base, Some(self.resolver)))
                            || init_type.is_compatible_with_refined_with_args(
                                &declared,
                                Some(self.resolver),
                            )
                    } else {
                        init_type
                            .is_compatible_with_refined_with_args(&declared, Some(self.resolver))
                    };
                    // v1.4.5 W1: an in-range INT LITERAL coerces to a narrow
                    // integer target (`let a: Int8 = 100`). The actual range
                    // proof lives in `check_range_and_measure_assignment`,
                    // which fires right below; a runtime Int value never
                    // narrows implicitly (Gate 7).
                    // Negative literals arrive as Unary('-', lit): peel so
                    // `let a: Int8 = -128` proves in-range too.
                    let literal_in_range = match init {
                        Expr::Literal(LiteralValue::Int(v), _) => {
                            DataraType::narrow_literal_fits(&declared, *v)
                        }
                        Expr::Unary { op, expr, .. } if op == "-" => match expr.as_ref() {
                            Expr::Literal(LiteralValue::Int(v), _) => {
                                DataraType::narrow_literal_fits(&declared, -v)
                            }
                            _ => false,
                        },
                        _ => false,
                    };
                    let compatible = compatible
                        || DataraType::is_compatible_narrow_literal(
                            &declared,
                            &init_type,
                            literal_in_range,
                        )
                        || DataraType::is_compatible_float_literal(
                            &declared,
                            &init_type,
                            matches!(init, Expr::Literal(LiteralValue::Float(_), _)),
                        );
                    if !compatible {
                        let help_msg = Self::suggest_type_fix(&declared, &init_type);
                        // M2 fix 4/5: a Str initializer where Int/Float is
                        // declared gets a wrap-in-str_to_int/str_to_float
                        // machine edit (source-sliced from the expression).
                        let machine_fix = self.str_parse_machine_fix(&declared, &init_type, init);
                        diag.error_with_fix(
                            ErrorCode::TypeMismatch,
                            format!(
                                "Type mismatch in variable declaration: expected '{}', got '{}'",
                                declared, init_type
                            ),
                            Some(span.clone()),
                            help_msg,
                            machine_fix,
                        );
                    }
                    self.check_range_and_measure_assignment(
                        &declared, &init_type, init, span, diag,
                    );
                    declared
                } else {
                    init_type
                };
                self.record_var_type(name, final_ty.clone());
                if let Expr::ListLiteral(elements, _) = init {
                    self.var_array_lengths.insert(name.clone(), elements.len());
                    if let Some(e) = self.last_list_element.take() {
                        self.var_element_types.insert(name.clone(), e);
                    }
                } else if let Expr::ArrayRepeatLiteral { count, .. } = init {
                    self.var_array_lengths.insert(name.clone(), *count);
                }
                self.symbol_mutability
                    .insert(name.clone(), MutabilityKind::Immutable);
                final_ty
            }
            Stmt::Mut {
                name,
                type_node,
                init,
                span,
            } => {
                // v1.5.0: publish the annotation while checking the init so
                // Outcome.err(..) adopts the annotated payload
                // (`mut r: Outcome<Int> = Outcome.err("init")`).
                let saved_assign_target = self.current_assign_target.take();
                if let Some(tn) = type_node {
                    self.current_assign_target = Some(self.resolve_type_node(tn, diag));
                }
                let init_type = self.check_expr(init, diag);
                self.current_assign_target = saved_assign_target;
                let final_ty = if let Some(tn) = type_node {
                    self.var_refinements.insert(name.clone(), tn.clone());
                    self.check_refinement(tn, init, span, diag);
                    let declared = self.resolve_type_node(tn, diag);
                    let is_literal_numeric = matches!(
                        init,
                        Expr::Literal(LiteralValue::Float(_), _)
                            | Expr::Literal(LiteralValue::Int(_), _)
                    );
                    let compatible = if let DataraType::Measure { base, .. } = &declared {
                        (is_literal_numeric
                            && init_type.is_compatible_with_args(base, Some(self.resolver)))
                            || init_type.is_compatible_with_refined_with_args(
                                &declared,
                                Some(self.resolver),
                            )
                    } else {
                        init_type
                            .is_compatible_with_refined_with_args(&declared, Some(self.resolver))
                    };
                    // v1.4.5 W1: an in-range INT LITERAL coerces to a narrow
                    // integer target (`let a: Int8 = 100`). The actual range
                    // proof lives in `check_range_and_measure_assignment`,
                    // which fires right below; a runtime Int value never
                    // narrows implicitly (Gate 7).
                    // Negative literals arrive as Unary('-', lit): peel so
                    // `let a: Int8 = -128` proves in-range too.
                    let literal_in_range = match init {
                        Expr::Literal(LiteralValue::Int(v), _) => {
                            DataraType::narrow_literal_fits(&declared, *v)
                        }
                        Expr::Unary { op, expr, .. } if op == "-" => match expr.as_ref() {
                            Expr::Literal(LiteralValue::Int(v), _) => {
                                DataraType::narrow_literal_fits(&declared, -v)
                            }
                            _ => false,
                        },
                        _ => false,
                    };
                    let compatible = compatible
                        || DataraType::is_compatible_narrow_literal(
                            &declared,
                            &init_type,
                            literal_in_range,
                        )
                        || DataraType::is_compatible_float_literal(
                            &declared,
                            &init_type,
                            matches!(init, Expr::Literal(LiteralValue::Float(_), _)),
                        );
                    if !compatible {
                        let help_msg = Self::suggest_type_fix(&declared, &init_type);
                        // M2 fix 4/5: a Str initializer where Int/Float is
                        // declared gets a wrap-in-str_to_int/str_to_float
                        // machine edit (source-sliced from the expression).
                        let machine_fix = self.str_parse_machine_fix(&declared, &init_type, init);
                        diag.error_with_fix(
                            ErrorCode::TypeMismatch,
                            format!(
                                "Type mismatch in variable declaration: expected '{}', got '{}'",
                                declared, init_type
                            ),
                            Some(span.clone()),
                            help_msg,
                            machine_fix,
                        );
                    }
                    self.check_range_and_measure_assignment(
                        &declared, &init_type, init, span, diag,
                    );
                    declared
                } else {
                    init_type
                };
                self.record_var_type(name, final_ty.clone());
                if let Expr::ListLiteral(elements, _) = init {
                    self.var_array_lengths.insert(name.clone(), elements.len());
                    if let Some(e) = self.last_list_element.take() {
                        self.var_element_types.insert(name.clone(), e);
                    }
                } else if let Expr::ArrayRepeatLiteral { count, .. } = init {
                    self.var_array_lengths.insert(name.clone(), *count);
                }
                self.symbol_mutability
                    .insert(name.clone(), MutabilityKind::MutableFixed);
                final_ty
            }
            Stmt::Val {
                name,
                type_node,
                init,
                is_mut,
                span,
            } => {
                self.symbol_decl_spans.insert(name.clone(), span.clone());
                // v1.5.0: publish the annotation while checking the initializer
                // so static constructors adopt it (`val m: Map<Int, Int> =
                // Map.new()`). Mirrors Stmt::Mut.
                let saved_assign_target = self.current_assign_target.take();
                if let Some(tn) = type_node {
                    self.current_assign_target = Some(self.resolve_type_node(tn, diag));
                }
                let init_type = self.check_expr(init, diag);
                self.current_assign_target = saved_assign_target;
                let final_ty = if let Some(tn) = type_node {
                    self.var_refinements.insert(name.clone(), tn.clone());
                    self.check_refinement(tn, init, span, diag);
                    let declared = self.resolve_type_node(tn, diag);
                    let is_literal_numeric = matches!(
                        init,
                        Expr::Literal(LiteralValue::Float(_), _)
                            | Expr::Literal(LiteralValue::Int(_), _)
                    );
                    let compatible = if let DataraType::Measure { base, .. } = &declared {
                        (is_literal_numeric
                            && init_type.is_compatible_with_args(base, Some(self.resolver)))
                            || init_type.is_compatible_with_refined_with_args(
                                &declared,
                                Some(self.resolver),
                            )
                    } else {
                        init_type
                            .is_compatible_with_refined_with_args(&declared, Some(self.resolver))
                    };
                    // v1.4.5 W1: an in-range INT LITERAL coerces to a narrow
                    // integer target (`let a: Int8 = 100`). The actual range
                    // proof lives in `check_range_and_measure_assignment`,
                    // which fires right below; a runtime Int value never
                    // narrows implicitly (Gate 7).
                    // Negative literals arrive as Unary('-', lit): peel so
                    // `let a: Int8 = -128` proves in-range too.
                    let literal_in_range = match init {
                        Expr::Literal(LiteralValue::Int(v), _) => {
                            DataraType::narrow_literal_fits(&declared, *v)
                        }
                        Expr::Unary { op, expr, .. } if op == "-" => match expr.as_ref() {
                            Expr::Literal(LiteralValue::Int(v), _) => {
                                DataraType::narrow_literal_fits(&declared, -v)
                            }
                            _ => false,
                        },
                        _ => false,
                    };
                    let compatible = compatible
                        || DataraType::is_compatible_narrow_literal(
                            &declared,
                            &init_type,
                            literal_in_range,
                        )
                        || DataraType::is_compatible_float_literal(
                            &declared,
                            &init_type,
                            matches!(init, Expr::Literal(LiteralValue::Float(_), _)),
                        );
                    if !compatible {
                        let help_msg = Self::suggest_type_fix(&declared, &init_type);
                        // M2 fix 4/5: a Str initializer where Int/Float is
                        // declared gets a wrap-in-str_to_int/str_to_float
                        // machine edit (source-sliced from the expression).
                        let machine_fix = self.str_parse_machine_fix(&declared, &init_type, init);
                        diag.error_with_fix(
                            ErrorCode::TypeMismatch,
                            format!(
                                "Type mismatch in variable declaration: expected '{}', got '{}'",
                                declared, init_type
                            ),
                            Some(span.clone()),
                            help_msg,
                            machine_fix,
                        );
                    }
                    self.check_range_and_measure_assignment(
                        &declared, &init_type, init, span, diag,
                    );
                    declared
                } else if !*is_mut {
                    // Type promotion: immutable `val` promotes directly to its concrete scalar SSA register type
                    init_type
                } else {
                    // Comptime Adaptive SSA Flow Specialization:
                    // Specialize to unboxed native register representation for 0-boxing speed.
                    init_type
                };
                self.record_var_type(name, final_ty.clone());
                if let Expr::ListLiteral(elements, _) = init {
                    self.var_array_lengths.insert(name.clone(), elements.len());
                    if let Some(e) = self.last_list_element.take() {
                        self.var_element_types.insert(name.clone(), e);
                    }
                } else if let Expr::ArrayRepeatLiteral { count, .. } = init {
                    self.var_array_lengths.insert(name.clone(), *count);
                }
                self.symbol_mutability
                    .insert(name.clone(), MutabilityKind::Dynamic { is_mut: *is_mut });
                final_ty
            }
            Stmt::CompactBind { name, init, .. } => {
                let init_type = self.check_expr(init, diag);
                self.record_var_type(name, init_type.clone());
                if let Expr::ListLiteral(elements, _) = init {
                    self.var_array_lengths.insert(name.clone(), elements.len());
                    if let Some(e) = self.last_list_element.take() {
                        self.var_element_types.insert(name.clone(), e);
                    }
                } else if let Expr::ArrayRepeatLiteral { count, .. } = init {
                    self.var_array_lengths.insert(name.clone(), *count);
                }
                self.symbol_mutability
                    .insert(name.clone(), MutabilityKind::MutableFixed);
                init_type
            }
            Stmt::Assign {
                target,
                value,
                span,
            } => {
                // v1.5.0: publish the LHS type while checking the RHS so
                // Outcome.err(..)/Maybe.none(..) in the assigned expression
                // adopt the target's payload (checked-API init pattern).
                let saved_assign_target = self.current_assign_target.take();
                self.current_assign_target = match target {
                    Expr::Identifier(name, _) => self.symbol_types.get(name).cloned(),
                    _ => None,
                };
                let val_type = self.check_expr(value, diag);
                self.current_assign_target = saved_assign_target;
                if let Expr::Identifier(name, _) = target {
                    // Keep the tracked list length in step with a literal
                    // reassignment, so a later `xs[i]` is not checked against
                    // the length of the value that was replaced.
                    match &value {
                        Expr::ListLiteral(elements, _) => {
                            self.var_array_lengths.insert(name.clone(), elements.len());
                        }
                        Expr::ArrayRepeatLiteral { count, .. } => {
                            self.var_array_lengths.insert(name.clone(), *count);
                        }
                        _ => {}
                    }
                    if let Some(tn) = self.var_refinements.get(name).cloned() {
                        self.check_refinement(&tn, value, span, diag);
                    }
                    if let Some(mut_kind) = self.symbol_mutability.get(name) {
                        match mut_kind {
                            MutabilityKind::Immutable => {
                                diag.error_with_help(
                                    ErrorCode::BorrowCannotMutateImmutable,
                                    format!("Cannot assign twice to immutable variable '{}'", name),
                                    Some(span.clone()),
                                    Some(format!(
                                        "consider declaring '{}' as mutable: 'mut {} = ...'",
                                        name, name
                                    )),
                                );
                            }
                            MutabilityKind::Dynamic { is_mut: false } => {
                                // Machine fix (M2 fix 2/5): rewrite the
                                // DECLARATION keyword `val` -> `mut` at the
                                // recorded declaration span. The assignment
                                // site itself needs no edit.
                                let decl_fix = self.symbol_decl_spans.get(name).map(|decl_span| {
                                    crate::diagnostics::MachineFix {
                                        span: decl_span.clone(),
                                        replacement: "mut".to_string(),
                                    }
                                });
                                diag.error_with_fix(
                                    ErrorCode::BorrowCannotMutateImmutable,
                                    format!(
                                        "Cannot assign to immutable val '{}'",
                                        name
                                    ),
                                    Some(span.clone()),
                                    Some(format!("'val' constants cannot be reassigned; declare the binding with 'mut {} = ..' if mutation is required", name)),
                                    decl_fix,
                                );
                            }
                            MutabilityKind::MutableFixed => {
                                if let Some(existing) = self.symbol_types.get(name).cloned() {
                                    self.check_range_and_measure_assignment(
                                        &existing, &val_type, value, span, diag,
                                    );
                                    if !val_type.is_compatible_with_refined_with_args(
                                        &existing,
                                        Some(self.resolver),
                                    ) {
                                        let help_msg = Self::suggest_type_fix(&existing, &val_type);
                                        // M2 fix 4/5: Str assigned where
                                        // Int/Float is expected gets the
                                        // wrap-in-parse machine edit.
                                        let machine_fix =
                                            self.str_parse_machine_fix(&existing, &val_type, value);
                                        diag.error_with_fix(
                                            ErrorCode::TypeMismatch,
                                            format!(
                                                "Type mismatch in assignment to mutable variable '{}': expected '{}', got '{}'",
                                                name, existing, val_type
                                            ),
                                            Some(span.clone()),
                                            help_msg,
                                            machine_fix,
                                        );
                                    }
                                }
                            }
                            MutabilityKind::Dynamic { is_mut: true } => {
                                if let Some(existing) = self.symbol_types.get(name).cloned() {
                                    self.check_range_and_measure_assignment(
                                        &existing, &val_type, value, span, diag,
                                    );
                                }
                                // Comptime Adaptive SSA Flow-Typing:
                                // Update symbol flow-type to newly assigned concrete type
                                // so downstream usages utilize unboxed registers with 0 tag check overhead.
                                self.symbol_types.insert(name.clone(), val_type.clone());
                            }
                        }
                    } else {
                        let mut candidates: Vec<&str> =
                            self.symbol_types.keys().map(|s| s.as_str()).collect();
                        candidates.sort_unstable();
                        let help_msg = if let Some(similar) =
                            crate::diagnostics::suggestions::find_best_match(name, candidates)
                        {
                            format!(
                                "a variable with a similar name exists: '{}'. Or declare with 'let {} = ...' / 'mut {} = ...'",
                                similar, name, name
                            )
                        } else {
                            format!(
                                "declare '{}' with 'let' (immutable) or 'mut' (mutable) before assigning to it",
                                name
                            )
                        };
                        diag.error_with_help(
                            ErrorCode::ResolveUndefinedSymbol,
                            format!("Assignment to undeclared variable '{}'", name),
                            Some(span.clone()),
                            Some(help_msg),
                        );
                    }
                } else {
                    let tgt_type = self.check_expr(target, diag);
                    if !val_type.is_compatible_with_args(&tgt_type, Some(self.resolver)) {
                        let help_msg = Self::suggest_type_fix(&tgt_type, &val_type);
                        // M2 fix 4/5: Str assigned where Int/Float is
                        // expected gets the wrap-in-parse machine edit.
                        let machine_fix = self.str_parse_machine_fix(&tgt_type, &val_type, value);
                        diag.error_with_fix(
                            ErrorCode::TypeMismatch,
                            format!(
                                "Type mismatch in assignment: expected '{}', got '{}'",
                                tgt_type, val_type
                            ),
                            Some(span.clone()),
                            help_msg,
                            machine_fix,
                        );
                    }
                }
                val_type
            }
            Stmt::Expr(e, _) => self.check_expr(e, diag),
            Stmt::Out(e, span) | Stmt::Err(e, span) => {
                let ty = self.check_expr(e, diag);
                if !self.is_printable_type(&ty) {
                    let help = match &ty {
                        DataraType::Class(c) => {
                            format!("add '@derive(Display)' to '{}' or use 'to_str(...)'", c)
                        }
                        DataraType::List(_) => {
                            "format list elements via a loop, e.g. 'for x in xs { out x }', or serialize with a custom formatter".to_string()
                        }
                        DataraType::Map { .. } => {
                            "format entries via a loop, or convert keys/values to Str".to_string()
                        }
                        _ => "provide an explicit conversion to Str".to_string(),
                    };
                    diag.error_with_help(
                        ErrorCode::UnprintableType,
                        format!("Cannot print value of unprintable type '{}'", ty),
                        Some(span.clone()),
                        Some(help),
                    );
                }
                DataraType::Unit
            }
            Stmt::Return(opt_e, span) => {
                let t = if let Some(e) = opt_e {
                    self.check_expr(e, diag)
                } else {
                    DataraType::Unit
                };
                // When the enclosing signature is Result/Option-like, the
                // returned value must be the same kind with a compatible
                // payload — there is no implicit wrapping or coercion.
                if let Some(expected) = self.current_return_type.clone() {
                    if expected != DataraType::Unit && opt_e.is_none() {
                        diag.error_with_help(
                            ErrorCode::TypeMissingReturn,
                            format!(
                                "Return statement has no value, but function declares return type '{}'",
                                expected
                            ),
                            Some(span.clone()),
                            Some(format!("return an expression of type '{}'", expected)),
                        );
                    }
                    let enc_res = expected.result_like();
                    let enc_opt = expected.option_like();
                    if enc_res.is_some() || enc_opt.is_some() {
                        // `return x?` propagates: the error path of the
                        // propagate-lowering returns the failed object
                        // unchanged and the success path re-wraps the
                        // payload in the enclosing constructor, so the
                        // returned payload type matching the signature is
                        // checked at the `?` site itself (E-TYPE-010 /
                        // payload compat there), not here.
                        let is_propagate_return = matches!(opt_e, Some(Expr::ErrorPropagate(..)));
                        let matches = if is_propagate_return {
                            true
                        } else if let Some((eok, eerr)) = &enc_res {
                            match t.result_like() {
                                Some((tok, terr)) => {
                                    tok.is_compatible_with_args(eok, Some(self.resolver))
                                        && terr.is_compatible_with_args(eerr, Some(self.resolver))
                                }
                                None => false,
                            }
                        } else if let Some(einner) = &enc_opt {
                            match t.option_like() {
                                Some(tinner) => {
                                    tinner.is_compatible_with_args(einner, Some(self.resolver))
                                }
                                None => false,
                            }
                        } else {
                            false
                        };
                        if !matches {
                            diag.error(
                                ErrorCode::TypeMismatch,
                                format!(
                                    "function signature returns '{}' but the return statement produces '{}'; construct it explicitly, e.g. Outcome<T> {{ is_success: .., value: .., error_msg: .. }}",
                                    expected, t
                                ),
                                Some(span.clone()),
                            );
                        }
                    } else if opt_e.is_some()
                        && expected != DataraType::Unit
                        && !matches!(expected, DataraType::TypeParam(_))
                        // v1.4.5 W1 parity with `let`: an in-range INT
                        // LITERAL coerces to a narrow integer return type
                        // (`fn f() -> UInt8 { return 0 }`). The actual range
                        // proof lives in the literal match below; a runtime
                        // Int value never narrows implicitly (Gate 7).
                        && !Self::return_is_narrow_literal(opt_e.as_ref(), &expected)
                        && !t.is_compatible_with_args(&expected, Some(self.resolver))
                    {
                        let help_msg = Self::suggest_type_fix(&expected, &t);
                        diag.error_with_help(
                            ErrorCode::TypeMismatch,
                            format!(
                                "Type mismatch in return statement: expected '{}', got '{}'",
                                expected, t
                            ),
                            Some(span.clone()),
                            help_msg,
                        );
                    }
                }
                t
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let cond_type = self.check_expr(condition, diag);
                if !cond_type.is_compatible_with_args(&DataraType::Bool, Some(self.resolver)) {
                    diag.error(
                        ErrorCode::TypeMismatch,
                        format!("Condition must be Bool, got '{}'", cond_type),
                        Some(span.clone()),
                    );
                }

                // Smart Type Narrowing for Option/Maybe:
                let is_neq_none = match condition {
                    Expr::Binary {
                        left, op, right, ..
                    } if op == "!=" => {
                        if let Expr::Identifier(var_name, _) = &**left {
                            if matches!(&**right, Expr::Literal(LiteralValue::None, _)) {
                                Some(var_name.clone())
                            } else {
                                None
                            }
                        } else if let Expr::Identifier(var_name, _) = &**right {
                            if matches!(&**left, Expr::Literal(LiteralValue::None, _)) {
                                Some(var_name.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                let is_eq_none = match condition {
                    Expr::Binary {
                        left, op, right, ..
                    } if op == "==" => {
                        if let Expr::Identifier(var_name, _) = &**left {
                            if matches!(&**right, Expr::Literal(LiteralValue::None, _)) {
                                Some(var_name.clone())
                            } else {
                                None
                            }
                        } else if let Expr::Identifier(var_name, _) = &**right {
                            if matches!(&**left, Expr::Literal(LiteralValue::None, _)) {
                                Some(var_name.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                if let Some(ref v) = is_neq_none {
                    let orig = self.symbol_types.get(v).cloned();
                    if let Some(DataraType::Option(inner)) = &orig {
                        self.symbol_types.insert(v.clone(), *inner.clone());
                    }
                    let res = self.check_stmt(then_branch, diag);
                    if let Some(o) = orig {
                        self.symbol_types.insert(v.clone(), o);
                    }
                    if let Some(eb) = else_branch {
                        let orig_else = self.symbol_types.get(v).cloned();
                        self.symbol_types
                            .insert(v.clone(), DataraType::Option(Box::new(DataraType::Unit)));
                        self.check_stmt(eb, diag);
                        if let Some(o) = orig_else {
                            self.symbol_types.insert(v.clone(), o);
                        }
                    }
                    res
                } else if let Some(ref v) = is_eq_none {
                    let orig = self.symbol_types.get(v).cloned();
                    self.symbol_types
                        .insert(v.clone(), DataraType::Option(Box::new(DataraType::Unit)));
                    let res = self.check_stmt(then_branch, diag);
                    if let Some(o) = orig {
                        self.symbol_types.insert(v.clone(), o);
                    }
                    if let Some(eb) = else_branch {
                        let orig_else = self.symbol_types.get(v).cloned();
                        if let Some(DataraType::Option(inner)) = &orig_else {
                            self.symbol_types.insert(v.clone(), *inner.clone());
                        }
                        self.check_stmt(eb, diag);
                        if let Some(o) = orig_else {
                            self.symbol_types.insert(v.clone(), o);
                        }
                    }
                    res
                } else if let Expr::IsCheck {
                    operand, ty_name, ..
                } = condition
                {
                    // M7 TSU narrowing: `if x is T` types x as T inside the
                    // then-branch (the degenerate TypeSet {T}) and keeps the
                    // dynamic type in the else-branch.
                    let narrowed = match &**operand {
                        Expr::Identifier(v, _) => {
                            let t = self.resolve_type_node(
                                &TypeNode {
                                    name: ty_name.clone(),
                                    generic_args: Vec::new(),
                                    is_option: false,
                                    error_type: None,
                                    refinement: None,
                                    span: crate::ast::SourceSpan::default(),
                                },
                                diag,
                            );
                            Some((v.clone(), t))
                        }
                        _ => None,
                    };
                    let res = if let Some((v, t)) = &narrowed {
                        let orig = self.symbol_types.get(v).cloned();
                        self.symbol_types.insert(v.clone(), t.clone());
                        let res = self.check_stmt(then_branch, diag);
                        if let Some(o) = orig {
                            self.symbol_types.insert(v.clone(), o);
                        }
                        res
                    } else {
                        self.check_stmt(then_branch, diag)
                    };
                    if let Some(eb) = else_branch {
                        self.check_stmt(eb, diag);
                    }
                    res
                } else {
                    let res = self.check_stmt(then_branch, diag);
                    if let Some(eb) = else_branch {
                        self.check_stmt(eb, diag);
                    }
                    res
                }
            }
            Stmt::For {
                var_name,
                iterable,
                body,
                ..
            } => {
                // `for` opens a breakable loop scope: `break` / `continue`
                // inside the body refer to this loop.
                self.loop_depth += 1;
                let res = self.check_for_arm(var_name, iterable, body, diag);
                self.loop_depth -= 1;
                res
            }
            Stmt::ParallelFor {
                var_name,
                iterable,
                body,
                ..
            } => {
                // A parallel body runs as concurrent tasks: `break` /
                // `continue` have no single control flow to target, so the
                // parallel scope does not open a breakable loop scope.
                self.check_for_arm(var_name, iterable, body, diag)
            }
            Stmt::While {
                condition,
                body,
                span,
            } => {
                let cond_type = self.check_expr(condition, diag);
                if !cond_type.is_compatible_with_args(&DataraType::Bool, Some(self.resolver)) {
                    diag.error(
                        ErrorCode::TypeMismatch,
                        format!("While condition must be Bool, got '{}'", cond_type),
                        Some(span.clone()),
                    );
                }
                self.loop_depth += 1;
                self.check_stmt(body, diag);
                self.loop_depth -= 1;
                DataraType::Unit
            }
            Stmt::Loop { body, .. } => {
                self.loop_depth += 1;
                self.check_stmt(body, diag);
                self.loop_depth -= 1;
                DataraType::Unit
            }
            Stmt::Break(span) => {
                if self.loop_depth == 0 {
                    diag.error(
                        ErrorCode::LoopControlOutsideLoop,
                        "'break' is only allowed inside a loop body (while/for/loop)".to_string(),
                        Some(span.clone()),
                    );
                }
                DataraType::Never
            }
            Stmt::Continue(span) => {
                if self.loop_depth == 0 {
                    diag.error(
                        ErrorCode::LoopControlOutsideLoop,
                        "'continue' is only allowed inside a loop body (while/for/loop)"
                            .to_string(),
                        Some(span.clone()),
                    );
                }
                DataraType::Never
            }
            Stmt::Parallel(body, _) => {
                self.check_stmt(body, diag);
                DataraType::Unit
            }
            Stmt::Simd(body, _) => {
                self.check_simd_loop_bounds(body, diag);
                self.check_stmt(body, diag);
                DataraType::Unit
            }
            Stmt::With {
                resource_name,
                init,
                body,
                ..
            } => {
                let init_type = self.check_expr(init, diag);
                if let Some(ref fn_name) = self.current_fn_name {
                    self.fn_symbol_types
                        .insert((fn_name.clone(), resource_name.clone()), init_type.clone());
                }
                let prev = self.symbol_types.insert(resource_name.clone(), init_type);
                let prev_mut = self
                    .symbol_mutability
                    .insert(resource_name.clone(), MutabilityKind::Immutable);
                self.check_stmt(body, diag);
                if let Some(p) = prev {
                    self.symbol_types.insert(resource_name.clone(), p);
                } else {
                    self.symbol_types.remove(resource_name);
                }
                if let Some(m) = prev_mut {
                    self.symbol_mutability.insert(resource_name.clone(), m);
                } else {
                    self.symbol_mutability.remove(resource_name);
                }
                DataraType::Unit
            }
            Stmt::Unsafe { body, .. } => {
                let prev_unsafe = self.in_unsafe;
                self.in_unsafe = true;
                self.check_stmt(body, diag);
                self.in_unsafe = prev_unsafe;
                DataraType::Unit
            }
            Stmt::Asm { structured, .. } => {
                // v1.4.0: every variable operand of a structured asm block
                // must be Int (i64); registers and immediates are typed by
                // the subset itself.
                for line in structured {
                    if let crate::ast::AsmLine::Inst { dst, src, .. } = line {
                        for opnd in [dst, src] {
                            if let crate::ast::AsmOperand::Var(name, vspan) = opnd {
                                if let Some(t) = self.symbol_types.get(name) {
                                    if !matches!(t, DataraType::Int) {
                                        diag.error(
                                            ErrorCode::AsmOperandNotInt,
                                            format!(
                                                "asm operand '{}' has type {:?}; asm blocks accept only Int (i64) variables",
                                                name, t
                                            ),
                                            Some(vspan.clone()),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                DataraType::Unit
            }
        }
    }
}
