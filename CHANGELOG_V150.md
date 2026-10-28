# Datara & forgen — 1.4.5 → 1.5.0 (одной сводкой)

## Ядро компилятора
- **TMM Layer 1 (dataflow lifetime inference)** — флагман 1.5.0: авто-освобождение
  на последнем использовании, авто-arena для циклов владения, escape-политика
  (release у владельца), инкрементальный lifetime-кэш v2, `forgen inspect --memory`.
  Zero runtime overhead (без счётчиков/барьеров — Clif-инспекция); @arc остаётся
  для shared ownership (+13–91% на аллокациях, замер bench_deref).
- **TSU-срез**: `is`-проверки по NaN-box тегу (8 Б val), branch narrowing,
  checked-конверсии, `List<any>`, E-ANY-001 на утечку динамики в статику.
- **M4 large-program**: стресс 200k строк — 115 с / 1.04 GiB; инкрементальный
  check-кэш с dependency fingerprints (prime 125 мс → hit 15–31 мс, bit-for-bit
  parity диагностик); symbol interning + E-LINK-001.
- **M8 performance**: loop-гэп ×13.9 устранён (OverflowElision — Datara быстрее
  C -O3 и rustc -O на 100M-петле и fib); LLVM systems-primitives (LNK2019-класс
  тихих фолбэков закрыт); PGO end-to-end (`forgen profile` → `--pgo-use`);
  compile-time стресса ×8.3 (200k: 115 с → 13.9 с).
- **Diagnostics**: 100% ErrorCode имеют explain с примерами; 4/5 machine-fixes
  (apply-and-recompile round-trip тесты); `forgen fix`.
- **LSP production**: hover/goto-def/references/rename, VS Code extension
  (grammar + 19 сниппетов), инкрементальный recheck < 50 мс.
- **NatVar срез 1**: стековые фиберы (Windows CreateFiberEx / POSIX ucontext),
  yield/sleep/join, каналы, 2000-фиберный дым, 75 мс на 10k spawn+drain.
- **Soundness-доработка**: 4 системных фикса (escape-семена nested-структур,
  field-view release-порядок, CFG-cycle guard, retype на AssignVar) — найдены
  через game_loop-детерминизм; + ретайп-гвардия DCE для fiber-функций.

## Stdlib и экосистема
- stdlib: collections sort/search/fold, text (UTF-8 кодпоинты), io.path +
  capability-guard, sys (env/exec), process spawn/pipes, diagnostics.D,
  web.ssr (XSS-safe SSR).
- dpm: publish→add→install→run e2e с merkle-верификацией и tamper-детектом;
  lock — детерминированный.
- Модули пишутся НА ДАТАРЕ (docs/STDLIB_ON_DATARA.md, гейт-тест).

## Безопасность и качество
- cargo-audit 0 CVE; fuzz-инварианты (парсер/чекер/манифест не паникуют на
  произвольных байтах); capability-матрица обходов; supply-chain тесты;
  SECURITY.md; CI security-audit джоба.
- 1509 тестов / 316 сьютов (было 1332/285); clippy --all-targets 0 warnings.

## Известные ограничения 1.5.0 (честно)
- ASan-прогон полного набора — post-1.5.0 (инструментарий ограничен хостом).
- NatVar: однопоточный планировщик, IO-поллер и tokio-сравнение — срез 2.
- SDL2/raylib-мосты — по решению владельца о тулчейн-зависимостях.
- coverage-guided fuzzing (cargo-fuzz) — под Linux CI.
