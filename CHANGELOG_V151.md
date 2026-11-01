# Datara / Forgen 1.5.1 «Consolidator» — Changelog

Дата: 2026-11-01. База: 1.5.0 «Conqueror» (0b660c5).
План: PLAN_V151.md (фазы N0–N7). Гейт: см. конец файла.

## Кратко

1.5.1 — фаза консолидации: гигиена релиза, реестр багов с честной
бэкенд-матрицей, многопоточный планировщик NatVar, экосистема пакетов
с Ed25519-подписью, флагманское демо «хаб в одном файле», conformance
четырёх уровней контроля.

## N0 — Гигиена

- Артефакты верификации разложены в `evidence/` (m4/, m5_deref/, m6/,
  natvar/, probes/, bench_python/, parity_matrix.json) + README-карта.
- Кириллица в .dtr верифицирована probe'ом.

## N1 — Реестр багов (7+ реальных багов, все с регресс-тестами)

- `datara_rt_str_scalar_at_cp` — кодпоинт-индексация строк
  («χαίρετε»[1] == «α» на всех тирах).
- `List.new(n)` static ctor (checker + lowering + resolver).
- **tools/parity_matrix.py** — дифф stdout Cranelift/JIT/LLVM; fallback
  бекенда считается провалом тира (ранее — ложный OK).
- **Каскад кастов** (коммит 6bd23da): DCE/inline теряли операнды
  Inst::Cast → E0901; infer без Expr::Cast → ISLE x64_lea паника на
  вложенных кастах; у LLVM не было ветки Inst::Cast (новый emit_cast.rs);
  str_scalar_at/_cp без lowering-типа (ret i64 в ptr-fn) и без
  emit_call-маппинга.
- **Каскад gcd/abs**: emit_call угонял имена пользовательских функций
  (guard module.functions); type-aware Int-abs → datara_rt_math_abs_int.
- No-op Float→Float каст: identity-пути Cranelift сохраняют носитель
  f64/f32 (ISLE-паника).
- method-return Outcome на LLVM: **корневой баг раскладки полей** —
  lowering сортировал поля литералов алфавитно, reader/C-ABI ждут
  declared-порядок → `or`-unwrap возвращал is_success(1) как значение.
  Канонический порядок = declared (decl_field_order), `record`-классы —
  sorted-fallback. Коммит 41fd691.

## N2 — NatVar срез 2

- Многопоточный планировщик `datara_natvar_mt.c`: N воркеров,
  per-worker очереди + кража; API fiber_mt_*; 10k/100k задач корректны,
  ~76 мс/100k; тесты test_natvar_mt 3/3.

## N3 — Экосистема

- Пакеты **resultx** и **timex**; реестр 5/5; consumer-верификация
  (main5 + main_all) на 3 бекендах.
- **Ed25519-подпись**: `dpm keygen`/`trust`, detached `.sig` при
  publish, `dpm verify` с эскалацией Mismatch (signing.rs; 3/3 теста).
- **`forgen bridge check`** — статический аудит мостов: E-BRIDGE-001/002
  на точных спанах; C-заголовки + арность вызовов (3/3 теста).
- **Python hot-path аудит**: zero-copy доказан (ptr-identity +
  двусторонняя мутация, ~10.4 ГБ/с); честно: rebind ~215 мкс,
  now_precise_ms возвращает микросекунды.
- **stdlib.collections**: сьют 16 проверок; найден и зафикшен тихий баг
  (pop/peek на пустом → fail-loud panic).

## N7 — Hub & Showcase

- **Flagship pipeline_demo** (examples/showcase/pipeline_demo): fs-CSV с
  capability-манифестом → агрегация → SIMD f32x4 → numpy zero-copy →
  SSR HTTP. Golden — **один md5 на все 3 тира** (тест test_pipeline_demo).
- **LLVM получил весь polyglot-мост** (py/lua декларации + маппинги; до
  этого — молчаливый fallback на любой py-вызов).
- **Levels-верификация** (tests/test_levels.rs 4/4): Scripting /
  Enterprise (Outcome+`?`) / Systems (arena+hton) / Kernel (fence) —
  каждый уровень реально исполняется.
- float_to_str_prec в prelude/resolver; write_file/read_file алиасы
  (Level-3 resolution); parse_int/parse_float в LLVM.

## Инфраструктура

- 60KB-гейт: сплиты natvar_call.rs, emit_cast.rs, builtin_returns.rs,
  signing.rs, prelude_signatures.rs.
- DATARA_BRIDGE_TRACE — диагностика py-моста (datara_value.c/datara_py.c).

## Гейт 1.5.1

- cargo test --release: 318+ сьютов / 1500+ тестов, 0 failed
- clippy 0, fmt 0
- parity-матрица 6/6 (честная: fallback = FAIL тира)
- size-gate 60KB — зелёный
- golden pipeline_demo: байт-в-байт на 3 тирах
