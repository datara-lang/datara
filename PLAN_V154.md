# PLAN_V154 — «Full Audit + Zero Known Issues»

Цель: закрыть ВСЕ оставшиеся дыры, провести полный аудит каждой подсистемы
и довести до состояния «ноль известных проблем».

---

## PA. Полный аудит — зафиксированные проблемы + немедленные фиксы

- [ ] **PA1**: clippy — 0 warnings ✓ (уже чист)
- [ ] **PA2**: unused imports/vars в тестах — прогнать и почистить
- [ ] **PA3**: dead_code в src/ — прогнать и оценить
- [ ] **PA4**: все examples запускаются без паники на Cranelift (базовый)
- [ ] **PA5**: все docs/ файлы согласованы с реальным API

## PB. Стресс и стабильность

- [ ] **PB1**: стресс — 100 мусорных .dtr файлов → ни одного креша
  компилятора (только чистые E-диагностики)
- [ ] **PB2**: soak — bench_natvar echo 30 сек × 5 прогонов, RSS до/после
- [ ] **PB3**: детерминизм — determinism_matrix зелёный на Cranelift+JIT
  (LLVM-тир = E-блок)

## PC. Скорость — публикация

- [ ] **PC1**: bench_vs_rust/RESULTS.md ✓ (уже создан)
- [ ] **PC2**: hotspot-профилирование арифметики — найти ТОП-3 узких места

## PD. DX — удобство

- [ ] **PD1**: forgen new --template ✓ (уже сделано)
- [ ] **PD2**: .vscode/launch.json для отладки AOT-бинарей
- [ ] **PD3**: docs/DEBUGGING.md — «Отладка за 5 минут»

## PE. Ядро — то что осталось

- [ ] **PE1**: #14 echo — stability fix (double-wake guard сузил, полный
  фикс = E3)
- [ ] **PE2**: #15a-e LLVM-тир расхождения (5 репро из determinism_matrix)
- [ ] **PE3**: async sugar — async { } поверх FiberHandle
- [ ] **PE4**: #10b SOLVED ✓ — mut-глобалы через global_data_map
- [ ] **PE5**: IOCP/epoll — 1.5.4 (самый большой техпробел)

## DoD
cargo test --release 0 FAILED + fmt clean + clippy 0 + size-gate OK +
checklist 8/8 + determinism_matrix Cranelift+JIT 100%.
