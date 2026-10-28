# Security Policy — Datara / forgen

## Reporting a vulnerability

Сообщайте об уязвимостях через GitHub Security Advisories («Report a
vulnerability») приватно либо письмом мейнтейнеру (см. профиль репозитория).
Пожалуйста, не открывайте публичный issue для эксплуатируемой уязвимости.

**Что включить:** репродуктор (исходник .dtr / manifest / команду), ожидаемое
и фактическое поведение, версию `forgen --version`.

**SLA (voluntary):** подтверждение — 7 дней; оценка и план фикса — 30 дней;
фикс + релиз — 90 дней или раньше. Регресс-тест добавляется к каждому фиксу.

## Capability model

Datara — язык с изолированными эффектами. Файловая система, сеть, переменные
окружения, процессы и exec — **escalated-операции**, требующие явного
разрешения:

1. `datara.toml` `[capabilities]` проекта (fs-read/fs-write/net/exec/env);
2. `unsafe(justification: "...")` на месте вызова для escalated-операций —
   fail-closed E0940 без обоснования;
3. TMM-границы FFI: borrowed-параметры не переживают вызов (E-TMM-002).

Матрица протестированных попыток обхода — `tests/test_capability_bypass_audit.rs`
(path traversal `../`, обход через dir_list, proc_spawn без exec-манифеста,
escalated без justification; fail-closed исход для каждой).

## Compiler integrity

- **Backend verification**: каждый билд проходит backend-верификацию
  (исполнение скомпилированной программы обязано совпасть с контрактом).
- **Fuzz-инварианты**: парсер/чекер/манифест на произвольных байтах и
  обрезках исходника обязаны возвращать диагностику, не паниковать
  (`tests/test_fuzz_invariants.rs`; cargo-fuzz/libFuzzer недоступен под
  MSVC-хостом — инвариантные seeded-фаззеры закрывают тот же контракт;
  coverage-guided fuzzing — post-1.5.0 под Linux CI).
- **Supply chain**: `cargo audit` — чисто на момент релиза (0 уязвимостей,
  160 зависимостей, 2026-10-25); advisory-DB обновляется при каждом CI-прогоне.
- **unsafe**: 112 unsafe-блоков в Rust-хосте, ключевые кластеры
  (NearMemory, C-API, JIT-вход) документированы SAFETY-комментариями;
  полный гросс-аудит остатка — по мере касания файлов.

## Supported versions

- v1.5.x — поддержка безопасности (фиксы + патч-релизы).
- v1.4.x — best effort до выхода 1.6.
