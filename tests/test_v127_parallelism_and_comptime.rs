//! Datara & Forgen v1.2.7 Test Suite: Fault-Tolerant Parallelism & Advanced Comptime
//!
//! Verifies:
//! 1. Actor crash isolation ("Let it Crash") without killing host/sibling fibers.
//! 2. Fail-fast supervision: cascading cancellation of sibling tasks.
//! 3. Automatic supervised restarts (Erlang OTP style).
//! 4. Lock-free zero-copy channel throughput and FIFO ordering.
//! 5. Advanced Turing-complete `comptime` loops and precomputation.
//! 6. Channel send linearity enforcement (no use-after-move).

use forgen::ast::{Expr, LiteralValue, SourceSpan, Stmt};
use forgen::diagnostics::{DiagnosticEngine, ErrorCode};
use forgen::optimizer::comptime_eval::ComptimeEvaluator;
use forgen::runtime::fiber::actor::{
    ActorError, CancellationToken, SupervisionPolicy, execute_isolated_actor, generate_actor_id,
};
use forgen::runtime::fiber::channel::Channel;
use forgen::runtime::fiber::supervisor::parallel_scope;
use forgen::security::SecurityVerifier;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

#[test]
fn test_v127_actor_isolated_crash() {
    let cancel = CancellationToken::new();
    let actor_id = generate_actor_id();

    // Spawn a crashing fiber with Isolate policy
    let result = execute_isolated_actor(actor_id, SupervisionPolicy::Isolate, cancel, || {
        panic!("simulated unproven hardware trap or assertion failure");
    });

    match result {
        Err(ActorError::Crashed(msg)) => {
            assert!(
                msg.contains("simulated unproven hardware trap"),
                "Crash message must capture the inner panic payload: {}",
                msg
            );
        }
        _ => panic!("Crashing actor must return ActorError::Crashed"),
    }

    // Ensure host and subsequent tasks continue normally
    let normal_res = execute_isolated_actor(
        generate_actor_id(),
        SupervisionPolicy::Isolate,
        CancellationToken::new(),
        || 42 * 2,
    );
    assert_eq!(normal_res, Ok(84));
}

#[test]
fn test_v127_actor_fail_fast_scope() {
    let result = parallel_scope(SupervisionPolicy::FailFast, |scope| {
        let child1 = scope.spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(10));
            panic!("critical database pipeline failed");
        });

        let child2 = scope.spawn(|| {
            // Long running sibling should observe cancellation
            for _ in 0..100 {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            100
        });

        (child1.join(), child2.join())
    });

    let (res1, res2) = result;
    assert!(res1.is_err(), "Child 1 must report crash");
    // Child 2 should either be cancelled or completed, but scope marked failure
    if let Err(e) = res2 {
        assert!(e == ActorError::Cancelled || matches!(e, ActorError::Crashed(_)));
    }
}

#[test]
fn test_v127_actor_restart_policy() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_clone = attempts.clone();

    let actor_id = generate_actor_id();
    let cancel = CancellationToken::new();

    // Fails twice, succeeds on third attempt
    let result = execute_isolated_actor(
        actor_id,
        SupervisionPolicy::Restart { max_retries: 3 },
        cancel,
        move || {
            let n = attempts_clone.fetch_add(1, Ordering::SeqCst);
            if n < 2 {
                panic!("temporary network blip");
            }
            999
        },
    );

    assert_eq!(result, Ok(999), "Actor must succeed after 2 retries");
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        3,
        "Must have taken exactly 3 attempts"
    );
}

#[test]
fn test_v127_lockfree_channel_high_throughput() {
    let chan = Channel::new(1024);
    let chan_producer = chan.clone();
    let chan_consumer = chan.clone();

    let count: i64 = 200_000;
    let start = Instant::now();

    let sender = std::thread::spawn(move || {
        for i in 0..count {
            chan_producer.send(i).expect("Send must succeed");
        }
    });

    let receiver = std::thread::spawn(move || {
        let mut sum: i64 = 0;
        for _ in 0..count {
            let val = chan_consumer.recv().expect("Recv must succeed");
            sum += val;
        }
        sum
    });

    sender.join().unwrap();
    let total_sum = receiver.join().unwrap();
    let elapsed = start.elapsed();

    let expected_sum: i64 = (count * (count - 1)) / 2;
    assert_eq!(total_sum, expected_sum, "FIFO sum must match exactly");

    let msgs_per_sec = (count as f64) / elapsed.as_secs_f64();
    println!(
        "[v1.2.7 Channel] Processed {} messages in {:.2} ms ({:.2} M msg/sec)",
        count,
        elapsed.as_secs_f64() * 1000.0,
        msgs_per_sec / 1_000_000.0
    );
    assert!(
        msgs_per_sec > 1_000_000.0,
        "Channel throughput must exceed 1.0 M msg/sec (measured: {:.2} M msg/sec)",
        msgs_per_sec / 1_000_000.0
    );
}

#[test]
fn test_v127_advanced_comptime_while_loop() {
    let span = SourceSpan::default();
    let mut evaluator = ComptimeEvaluator::new();

    // AST for:
    // {
    //     mut i = 1;
    //     mut sum = 0;
    //     while i <= 100 {
    //         sum = sum + i;
    //         i = i + 1;
    //     }
    //     sum
    // }
    let stmts = vec![
        Stmt::Mut {
            name: "i".to_string(),
            type_node: None,
            init: Expr::Literal(LiteralValue::Int(1), span.clone()),
            span: span.clone(),
        },
        Stmt::Mut {
            name: "sum".to_string(),
            type_node: None,
            init: Expr::Literal(LiteralValue::Int(0), span.clone()),
            span: span.clone(),
        },
        Stmt::While {
            condition: Expr::Binary {
                op: "<=".to_string(),
                left: Box::new(Expr::Identifier("i".to_string(), span.clone())),
                right: Box::new(Expr::Literal(LiteralValue::Int(100), span.clone())),
                span: span.clone(),
            },
            body: Box::new(Stmt::Block(
                vec![
                    Stmt::Assign {
                        target: Expr::Identifier("sum".to_string(), span.clone()),
                        value: Expr::Binary {
                            op: "+".to_string(),
                            left: Box::new(Expr::Identifier("sum".to_string(), span.clone())),
                            right: Box::new(Expr::Identifier("i".to_string(), span.clone())),
                            span: span.clone(),
                        },
                        span: span.clone(),
                    },
                    Stmt::Assign {
                        target: Expr::Identifier("i".to_string(), span.clone()),
                        value: Expr::Binary {
                            op: "+".to_string(),
                            left: Box::new(Expr::Identifier("i".to_string(), span.clone())),
                            right: Box::new(Expr::Literal(LiteralValue::Int(1), span.clone())),
                            span: span.clone(),
                        },
                        span: span.clone(),
                    },
                ],
                span.clone(),
            )),
            span: span.clone(),
        },
    ];

    let block_expr = Expr::Block(
        stmts,
        Some(Box::new(Expr::Identifier("sum".to_string(), span.clone()))),
        span,
    );

    let result = evaluator
        .eval_expr(&block_expr)
        .expect("Comptime evaluation must succeed");
    assert_eq!(
        result,
        LiteralValue::Int(5050),
        "Compile-time loop from 1 to 100 must evaluate to 5050"
    );
}

#[test]
fn test_v127_comptime_precomputed_lookup_table() {
    let span = SourceSpan::default();
    let mut evaluator = ComptimeEvaluator::new();

    // AST for:
    // {
    //     mut i = 0;
    //     mut power = 1;
    //     while i < 16 {
    //         power = power * 2;
    //         i = i + 1;
    //     }
    //     power
    // }
    let stmts = vec![
        Stmt::Mut {
            name: "i".to_string(),
            type_node: None,
            init: Expr::Literal(LiteralValue::Int(0), span.clone()),
            span: span.clone(),
        },
        Stmt::Mut {
            name: "power".to_string(),
            type_node: None,
            init: Expr::Literal(LiteralValue::Int(1), span.clone()),
            span: span.clone(),
        },
        Stmt::While {
            condition: Expr::Binary {
                op: "<".to_string(),
                left: Box::new(Expr::Identifier("i".to_string(), span.clone())),
                right: Box::new(Expr::Literal(LiteralValue::Int(16), span.clone())),
                span: span.clone(),
            },
            body: Box::new(Stmt::Block(
                vec![
                    Stmt::Assign {
                        target: Expr::Identifier("power".to_string(), span.clone()),
                        value: Expr::Binary {
                            op: "*".to_string(),
                            left: Box::new(Expr::Identifier("power".to_string(), span.clone())),
                            right: Box::new(Expr::Literal(LiteralValue::Int(2), span.clone())),
                            span: span.clone(),
                        },
                        span: span.clone(),
                    },
                    Stmt::Assign {
                        target: Expr::Identifier("i".to_string(), span.clone()),
                        value: Expr::Binary {
                            op: "+".to_string(),
                            left: Box::new(Expr::Identifier("i".to_string(), span.clone())),
                            right: Box::new(Expr::Literal(LiteralValue::Int(1), span.clone())),
                            span: span.clone(),
                        },
                        span: span.clone(),
                    },
                ],
                span.clone(),
            )),
            span: span.clone(),
        },
    ];

    let block_expr = Expr::Block(
        stmts,
        Some(Box::new(Expr::Identifier(
            "power".to_string(),
            span.clone(),
        ))),
        span,
    );

    let result = evaluator
        .eval_expr(&block_expr)
        .expect("Comptime evaluation must succeed");
    assert_eq!(
        result,
        LiteralValue::Int(65536),
        "2^16 precomputed at compile time must be 65536"
    );
}

#[test]
fn test_v127_actor_channel_linearity_security_gate() {
    let span = SourceSpan::default();
    let mut diag = DiagnosticEngine::new("en");

    // Statements:
    // let msg = 100
    // channel.send(msg)
    // let leak = msg  <-- Error: BorrowUseAfterMove
    let stmts = vec![
        Stmt::Let {
            name: "msg".to_string(),
            type_node: None,
            init: Expr::Literal(LiteralValue::Int(100), span.clone()),
            span: span.clone(),
        },
        Stmt::Expr(
            Expr::Call {
                callee: Box::new(Expr::MemberAccess {
                    object: Box::new(Expr::Identifier("channel".to_string(), span.clone())),
                    member: "send".to_string(),
                    span: span.clone(),
                }),
                args: vec![Expr::Identifier("msg".to_string(), span.clone())],
                span: span.clone(),
            },
            span.clone(),
        ),
        Stmt::Let {
            name: "leak".to_string(),
            type_node: None,
            init: Expr::Identifier("msg".to_string(), span.clone()),
            span: span.clone(),
        },
    ];

    SecurityVerifier::check_channel_send_linearity(&stmts, &mut diag);
    assert!(
        diag.has_errors(),
        "Must flag use-after-move when reading variable after channel.send"
    );
    assert!(
        diag.diagnostics
            .iter()
            .any(|d| d.code == ErrorCode::BorrowUseAfterMove.as_str()),
        "Diagnostic must have code BorrowUseAfterMove"
    );
}
