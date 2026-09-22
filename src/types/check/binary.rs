use crate::ast::*;
use crate::diagnostics::{DiagnosticEngine, ErrorCode, SourceSpan};
use crate::types::{DataraType, TypeChecker};

impl<'a> TypeChecker<'a> {
    pub(crate) fn check_binary(
        &mut self,
        op: &str,
        left: &Expr,
        right: &Expr,
        span: &SourceSpan,
        diag: &mut DiagnosticEngine,
    ) -> DataraType {
        let lt = self.check_expr(left, diag);
        let rt = self.check_expr(right, diag);

        // --- Shift amount validation (v1.3.2 bitwise operators; widened in
        // v1.4.5 W1 to the operand's own width). `Int` is a signed 64-bit
        // integer so its constant shift amount must lie in 0..64; a narrow
        // operand (Int8, UInt16, ...) masks its shift to its own width —
        // documented SPEC_V1 behavior — so the constant range check follows
        // the operand width. ---
        if matches!(op, "<<" | ">>") {
            let shift_width: i128 = if lt == DataraType::Int8 || lt == DataraType::UInt8 {
                8
            } else if lt == DataraType::Int16 || lt == DataraType::UInt16 {
                16
            } else if lt == DataraType::Int32 || lt == DataraType::UInt32 {
                32
            } else {
                64
            };
            if let Expr::Literal(LiteralValue::Int(amount), shift_span) = right {
                if *amount < 0 || (*amount as i128) >= shift_width {
                    diag.error(
                        ErrorCode::RangeViolation,
                        format!(
                            "Shift amount {} is out of range for a {}-bit shift: the shift count must be in 0..{} (Datara masks shifts to the operand width)",
                            amount, shift_width, shift_width
                        ),
                        Some(shift_span.clone()),
                    );
                }
            }
        }

        // --- Units of Measure Dimensional Analysis ---
        if let (
            DataraType::Measure { base: b1, unit: u1 },
            DataraType::Measure { base: b2, unit: u2 },
        ) = (&lt, &rt)
        {
            let base = if **b1 == DataraType::Float || **b2 == DataraType::Float {
                DataraType::Float
            } else {
                DataraType::Int
            };
            match op {
                "+" | "-" => {
                    if u1 != u2 {
                        diag.error(
                                    ErrorCode::DimensionMismatch,
                                    format!(
                                        "Cannot perform '{}' on incompatible units of measure '{}' and '{}'",
                                        op, u1, u2
                                    ),
                                    Some(span.clone()),
                                );
                    }
                    return DataraType::Measure {
                        base: Box::new(base),
                        unit: u1.clone(),
                    };
                }
                "*" => {
                    let unit = format!("{}*{}", u1, u2);
                    return DataraType::Measure {
                        base: Box::new(base),
                        unit,
                    };
                }
                "/" => {
                    if u1 == u2 {
                        return base;
                    } else {
                        let unit = format!("{}/{}", u1, u2);
                        return DataraType::Measure {
                            base: Box::new(base),
                            unit,
                        };
                    }
                }
                "==" | "!=" | "<" | "<=" | ">" | ">=" => {
                    if u1 != u2 {
                        diag.error(
                            ErrorCode::DimensionMismatch,
                            format!(
                                "Cannot compare incompatible units of measure '{}' and '{}'",
                                u1, u2
                            ),
                            Some(span.clone()),
                        );
                    }
                    return DataraType::Bool;
                }
                _ => {}
            }
        } else if let DataraType::Measure { base, unit } = &lt {
            match op {
                "*" => {
                    let b = if **base == DataraType::Float || rt == DataraType::Float {
                        DataraType::Float
                    } else {
                        DataraType::Int
                    };
                    return DataraType::Measure {
                        base: Box::new(b),
                        unit: unit.clone(),
                    };
                }
                "/" => {
                    let b = if **base == DataraType::Float || rt == DataraType::Float {
                        DataraType::Float
                    } else {
                        DataraType::Int
                    };
                    return DataraType::Measure {
                        base: Box::new(b),
                        unit: unit.clone(),
                    };
                }
                "+" | "-" => {
                    diag.error(
                                ErrorCode::DimensionMismatch,
                                format!(
                                    "Cannot perform '{}' between unit of measure '{}' and dimensionless quantity",
                                    op, unit
                                ),
                                Some(span.clone()),
                            );
                    return DataraType::Measure {
                        base: base.clone(),
                        unit: unit.clone(),
                    };
                }
                "==" | "!=" | "<" | "<=" | ">" | ">=" => {
                    if matches!(
                        right,
                        Expr::Literal(LiteralValue::Int(_) | LiteralValue::Float(_), _)
                    ) {
                        return DataraType::Bool;
                    }
                    diag.error(
                        ErrorCode::DimensionMismatch,
                        format!(
                            "Cannot compare unit of measure '{}' with dimensionless quantity",
                            unit
                        ),
                        Some(span.clone()),
                    );
                    return DataraType::Bool;
                }
                _ => {}
            }
        } else if let DataraType::Measure { base, unit } = &rt {
            match op {
                "*" => {
                    let b = if lt == DataraType::Float || **base == DataraType::Float {
                        DataraType::Float
                    } else {
                        DataraType::Int
                    };
                    return DataraType::Measure {
                        base: Box::new(b),
                        unit: unit.clone(),
                    };
                }
                "+" | "-" => {
                    diag.error(
                                ErrorCode::DimensionMismatch,
                                format!(
                                    "Cannot perform '{}' between dimensionless quantity and unit of measure '{}'",
                                    op, unit
                                ),
                                Some(span.clone()),
                            );
                    return DataraType::Measure {
                        base: base.clone(),
                        unit: unit.clone(),
                    };
                }
                "==" | "!=" | "<" | "<=" | ">" | ">=" => {
                    if matches!(
                        left,
                        Expr::Literal(LiteralValue::Int(_) | LiteralValue::Float(_), _)
                    ) {
                        return DataraType::Bool;
                    }
                    diag.error(
                        ErrorCode::DimensionMismatch,
                        format!(
                            "Cannot compare dimensionless quantity with unit of measure '{}'",
                            unit
                        ),
                        Some(span.clone()),
                    );
                    return DataraType::Bool;
                }
                _ => {}
            }
        }

        // --- Range Interval Arithmetic ---
        if let (
            DataraType::Range {
                base: b1,
                min: min1,
                max: max1,
            },
            DataraType::Range {
                base: b2,
                min: min2,
                max: max2,
            },
        ) = (&lt, &rt)
        {
            let base = if **b1 == DataraType::Float || **b2 == DataraType::Float {
                DataraType::Float
            } else {
                DataraType::Int
            };
            match op {
                "+" => {
                    return DataraType::Range {
                        base: Box::new(base),
                        min: min1.saturating_add(*min2),
                        max: max1.saturating_add(*max2),
                    };
                }
                "-" => {
                    return DataraType::Range {
                        base: Box::new(base),
                        min: min1.saturating_sub(*max2),
                        max: max1.saturating_sub(*min2),
                    };
                }
                "*" => {
                    let p1 = min1.saturating_mul(*min2);
                    let p2 = min1.saturating_mul(*max2);
                    let p3 = max1.saturating_mul(*min2);
                    let p4 = max1.saturating_mul(*max2);
                    let min = p1.min(p2).min(p3).min(p4);
                    let max = p1.max(p2).max(p3).max(p4);
                    return DataraType::Range {
                        base: Box::new(base),
                        min,
                        max,
                    };
                }
                "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||" => {
                    return DataraType::Bool;
                }
                _ => {}
            }
        } else if let DataraType::Range { base, min, max } = &lt {
            if let Expr::Literal(LiteralValue::Int(n), _) = right {
                let val = *n as i128;
                match op {
                    "+" => {
                        return DataraType::Range {
                            base: base.clone(),
                            min: min.saturating_add(val),
                            max: max.saturating_add(val),
                        };
                    }
                    "-" => {
                        return DataraType::Range {
                            base: base.clone(),
                            min: min.saturating_sub(val),
                            max: max.saturating_sub(val),
                        };
                    }
                    "*" => {
                        let p1 = min.saturating_mul(val);
                        let p2 = max.saturating_mul(val);
                        return DataraType::Range {
                            base: base.clone(),
                            min: p1.min(p2),
                            max: p1.max(p2),
                        };
                    }
                    "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||" => {
                        return DataraType::Bool;
                    }
                    _ => {}
                }
            }
        } else if let DataraType::Range { base, min, max } = &rt
            && let Expr::Literal(LiteralValue::Int(n), _) = left
        {
            let val = *n as i128;
            match op {
                "+" => {
                    return DataraType::Range {
                        base: base.clone(),
                        min: val.saturating_add(*min),
                        max: val.saturating_add(*max),
                    };
                }
                "-" => {
                    return DataraType::Range {
                        base: base.clone(),
                        min: val.saturating_sub(*max),
                        max: val.saturating_sub(*min),
                    };
                }
                "*" => {
                    let p1 = val.saturating_mul(*min);
                    let p2 = val.saturating_mul(*max);
                    return DataraType::Range {
                        base: base.clone(),
                        min: p1.min(p2),
                        max: p1.max(p2),
                    };
                }
                "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||" => {
                    return DataraType::Bool;
                }
                _ => {}
            }
        }

        // --- Operator operand validation ---
        // `Val` (the dynamic type), `TypeParam`s, `Measure`/`Range`
        // quantities and `Never` are intentionally left permissive:
        // they carry their own rules (dimensional analysis and
        // interval arithmetic above) or are resolved dynamically.
        let is_open = |t: &DataraType| {
            matches!(
                t,
                DataraType::Val
                    | DataraType::TypeParam(_)
                    | DataraType::Never
                    | DataraType::Measure { .. }
                    | DataraType::Range { .. }
            )
        };
        let is_numeric = |t: &DataraType| {
            matches!(
                t,
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
                    | DataraType::Dec64
            )
        };
        let is_orderable = |t: &DataraType| {
            matches!(
                t,
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
                    | DataraType::Dec64
                    | DataraType::String
                    | DataraType::Char
            )
        };
        let report_bad_operands = |diag: &mut DiagnosticEngine| {
            diag.error(
                ErrorCode::TypeInvalidBinaryOp,
                format!(
                    "Operator '{}' cannot be applied to operands of type '{}' and '{}'",
                    op, lt, rt
                ),
                Some(span.clone()),
            );
        };

        // --- SIMD Vector Operations & Strict Type Isolation (v1.4.5) ---
        if lt == DataraType::SimdF32x4
            || rt == DataraType::SimdF32x4
            || lt == DataraType::SimdI32x4
            || rt == DataraType::SimdI32x4
        {
            if lt != rt {
                diag.error_with_help(
                    ErrorCode::TypeIncomparableOperands,
                    format!("SIMD vector type mismatch: cannot combine '{}' and '{}'", lt, rt),
                    Some(span.clone()),
                    Some("Datara enforces strict SIMD type isolation (E-TYPE-008): use explicit conversion '.to_f32()' or '.to_i32()'.".to_string()),
                );
                return if lt == DataraType::SimdF32x4 || rt == DataraType::SimdF32x4 {
                    DataraType::SimdF32x4
                } else {
                    DataraType::SimdI32x4
                };
            }
            match op {
                "+" | "-" | "*" | "/" => return lt,
                "&" | "|" | "^" if lt == DataraType::SimdI32x4 => return lt,
                "==" | "!=" => return DataraType::Bool,
                _ => {
                    diag.error(
                        ErrorCode::TypeInvalidBinaryOp,
                        format!(
                            "Operator '{}' cannot be applied to SIMD vector type '{}'",
                            op, lt
                        ),
                        Some(span.clone()),
                    );
                    return lt;
                }
            }
        }

        if !is_open(&lt) && !is_open(&rt) {
            match op {
                "+" | "-" | "*" | "/" | "%" => {
                    // Str concatenation via `+` is an intended
                    // language feature and stays permissive.
                    let concat =
                        op == "+" && (lt == DataraType::String || rt == DataraType::String);
                    if !concat {
                        if !is_numeric(&lt) || !is_numeric(&rt) {
                            report_bad_operands(diag);
                        } else if lt != rt {
                            // v1.4.5 W1: an INT LITERAL on one side adopts
                            // the other operand's integer width — literals
                            // are compile-time values, not runtime Ints,
                            // so this is literal labeling, not a Gate-7
                            // implicit conversion of a runtime value.
                            let lit_int =
                                |e: &Expr| matches!(e, Expr::Literal(LiteralValue::Int(_), _));
                            // v1.4.5 W2: a FLOAT literal adopts the other
                            // operand's float width the same way an int
                            // literal adopts an integer width.
                            let lit_float =
                                |e: &Expr| matches!(e, Expr::Literal(LiteralValue::Float(_), _));
                            let mixed = !((lit_int(left) && DataraType::is_integer_type(&rt)
                                || lit_int(right) && DataraType::is_integer_type(&lt))
                                || (lit_float(left) && DataraType::is_float_type(&rt))
                                || (lit_float(right) && DataraType::is_float_type(&lt)));
                            if mixed {
                                diag.error_with_help(
                                    ErrorCode::TypeIncomparableOperands,
                                    format!(
                                        "Arithmetic operator '{}' cannot combine operands of different numeric types '{}' and '{}'",
                                        op, lt, rt
                                    ),
                                    Some(span.clone()),
                                    Some("Datara never widens numeric types implicitly (SPEC_V1 Gate 7): use explicit cast 'as Float' or 'as Int'.".to_string()),
                                );
                            }
                        }
                    }
                }
                "==" | "!=" => {
                    // Bool/Int cross-comparisons used to be "intended
                    // dynamic behavior", but they contradict SPEC_V1
                    // Gate 5 (no truthy integers) and Gate 7 (no implicit
                    // numeric conversions). They are now flagged with a
                    // warning so existing code still compiles while the
                    // language converges on strict Bool==Bool equality.
                    let bool_int_cross = (lt == DataraType::Bool && rt == DataraType::Int)
                        || (lt == DataraType::Int && rt == DataraType::Bool);
                    if bool_int_cross {
                        diag.warning(
                            ErrorCode::BoolIntComparison,
                            format!(
                                "Comparing Bool with Int ('{} {} {}') relies on truthy-integer coercion, which Datara forbids elsewhere (Gate 5). Compare Bool with Bool, or convert explicitly.",
                                lt, op, rt
                            ),
                            Some(span.clone()),
                        );
                    }
                    let is_truthy_scalar = |t: &DataraType| {
                        matches!(
                            t,
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
                                | DataraType::Dec64
                                | DataraType::Bool
                        )
                    };
                    if !(is_truthy_scalar(&lt) && is_truthy_scalar(&rt))
                        && !lt.is_compatible_with_refined_with_args(&rt, Some(self.resolver))
                        && !rt.is_compatible_with_refined_with_args(&lt, Some(self.resolver))
                    {
                        report_bad_operands(diag);
                    }
                }
                "<" | "<=" | ">" | ">=" => {
                    if !is_orderable(&lt) || !is_orderable(&rt) {
                        report_bad_operands(diag);
                    } else if lt != rt {
                        // Strict ordering comparisons (SPEC_V1, Gate 7):
                        // both sides must carry the same orderable type.
                        // Each operand being orderable on its own is not
                        // enough — `i < 3.5` (Int vs Float) used to widen
                        // silently at lowering and `i < "three"` (Int vs
                        // Str) used to pass outright. Datara promises
                        // fail-closed semantics with no implicit
                        // conversions, so cross-type ordering is rejected.
                        let help = if is_numeric(&lt) && is_numeric(&rt) {
                            Some("Datara never widens numeric types implicitly (SPEC_V1 Gate 7): compare values of the same type, e.g. use a Float variable with float literals written with a decimal point, or compare Int with Int.".to_string())
                        } else if lt == DataraType::String || rt == DataraType::String {
                            Some("Str is not orderable against other types: compare Str with Str, or convert with 'str_to_int' / 'str_to_float' before comparing.".to_string())
                        } else {
                            Some(
                                "Ordering comparisons require two operands of the same type."
                                    .to_string(),
                            )
                        };
                        diag.error_with_help(
                            ErrorCode::TypeIncomparableOperands,
                            format!(
                                "Ordering operator '{}' cannot compare operands of type '{}' and '{}'",
                                op, lt, rt
                            ),
                            Some(span.clone()),
                            help,
                        );
                    }
                }
                "&&" | "||" => {
                    // Strict Bool logical operators (SPEC_V1 Gate 5):
                    // operands must be Bool, no truthy integer coercion.
                    if lt != DataraType::Bool || rt != DataraType::Bool {
                        report_bad_operands(diag);
                    }
                }
                "&" | "|" | "^" | "<<" | ">>" => {
                    // Bitwise operators (v1.3.2) are defined only on
                    // Int. No implicit numeric conversion (SPEC_V1
                    // Gate 7): `1.5 & 2` is a type error, not a truncation.
                    if lt != DataraType::Int || rt != DataraType::Int {
                        report_bad_operands(diag);
                    }
                }
                _ => {}
            }
        }

        match op {
            "+" if lt == DataraType::String || rt == DataraType::String => DataraType::String,
            "+" | "-" | "*" | "/" | "%" => {
                // v1.4.5 W1: the result carries the operand's width. Strict
                // same-type arithmetic (Gate 7) means lt == rt for numeric
                // operands, so the left operand's type IS the result type.
                if lt == DataraType::Float
                    || rt == DataraType::Float
                    || lt == DataraType::Float32
                    || rt == DataraType::Float32
                {
                    // Float dominates: Float32 op Float stays Float per the
                    // promotion-free rule (same-type enforcement above makes
                    // this the un-mixed case anyway).
                    if lt == DataraType::Float32 || rt == DataraType::Float32 {
                        DataraType::Float32
                    } else {
                        DataraType::Float
                    }
                } else {
                    lt
                }
            }
            "==" | "!=" | "<" | "<=" | ">" | ">=" | "&&" | "||" => DataraType::Bool,
            "&" | "|" | "^" | "<<" | ">>" => {
                // Bitwise results keep the operand's integer width too.
                lt
            }
            _ => lt,
        }
    }
}
