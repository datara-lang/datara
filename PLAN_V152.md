# PLAN_V152 — Datara/Forgen 1.5.2 «Multiplier» — ДО ИДЕАЛА

База: 1.5.1 «Consolidator» (fdc2f28, tag v1.5.1). Статус-гейт 1.5.1:
311+ сьютов 0 failed · fmt 0 · clippy 0 · parity 6/6 честная · size-gate ok ·
golden pipeline_demo один md5 на 3 тира · levels 4/4 · docs-гейт 4/4.

## Определение цели: что значит «8/10»

8/10 = **всё техническое закрыто, остаётся только community**:

| Слой | Сейчас | Цель 1.5.2 |
|---|---|---|
| Компилятор/корректность | 6/10 (дыры чекера известны) | 9/10 — ноль известных дыр |
| Конкурентность | 5/10 (нет IO, нет атомиков) | 8/10 — echo-сервер на фиберах работает |
| Стандартная библиотека | 5/10 (фрагменты) | 8/10 — net/testing/math укомплектованы, каждый модуль с тестами |
| Тулинг | 6/10 (LSP/fmt/watch есть, отладчик нет) | 8/10 — debug-профиль + LSP × 1.5 |
| Дистрибуция | 6/10 (winget PR, 17 артефактов) | 9/10 — все реестры, «релиз одной командой» |
| Доки | 5/10 (мануал × 1.5 готов) | 8/10 — docs-гейт на ВСЕ секции, тур обновлён |
| Экосистема | 2/10 | 6/10 — 3 demo-проекта + playground + Pages-реестр |
| Community | 1/10 | 2/10 — НЕ ЦЕЛЬ 1.5.2 (единственное, что останется) |

---

## N0. Гигиена `[ ]` (1 сессия)

- [ ] CI-флейк на main: прогнать флейк-лист (test_v120_soa, bce-подобные),
  зафиксировать повтор-правила; зелёный main — обязательное условие релиза.
- [ ] check/expr.rs (61.5KB) — экстракция Cast-checker в
  `types/check/cast.rs`; снять allow-list запись.
- [ ] Убрать отладочные хвосты: `DATARA_BRIDGE_TRACE` оставить (env-gated,
  документирован), лишние eprintln — вычистить.
- [ ] Ротация GitHub-токена (владелец).

## N1. Конкурентность до конца: NatVar срез 3 `[!]` (2–3 сессии, САМОЕ ВАЖНОЕ)

### 1.1 IO-поллер
- [ ] Windows IOCP / Linux epoll в отдельном потоке; парковка фибера на
  сокете: EWOULDBLOCK → park → wake (таймер-колесо уже умеет).
- [ ] `fiber_net_recv(sock, buf, timeout_ms) -> Outcome<Int>`,
  `fiber_net_send(...)` — реестры во всех трёх бекендах + resolver + prelude.
- [ ] Тест: echo-сервер на фиберах — 100 соединений, 1000 сообщений,
  ноль блокирующих зависаний (test_natvar_io.rs).
- [ ] sockets → stdlib.net: TcpListener.accept() на фибере.

### 1.2 Атомики и синхронизация
- [x] `AtomicInt` builtin — ЗАКРЫТО 2026-11-01: atomic_new/load/store/
  fetch_add/cas (C: Interlocked на Windows, __sync на POSIX; слот —
  heap-ячейка 8 байт). Проведено во ВСЕ тиры (declare_ext, jit_externs,
  jit, llvm decls+map, resolver, prelude_signatures, builtin_returns).
  Тест test_atomics: 8 MT-воркеров × 1250 fetch_add = РОВНО 10000 +
  CAS-семантика (старое значение) — 10000/5/5/500.
- [ ] `rt_mutex_lock -> Outcome` (err="timeout") — неблокирующая обёртка.
- [ ] `--detect-deadlock` (OFF по умолчанию): граф ожидания фиберов,
  цикл → диагностика с путём. Тест: AB-BA ловится < 1 с.

### 1.3 stdlib.async — публичная поверхность
- [ ] `FiberHandle`-обёртка: `join() -> Outcome<T>`, `is_done()`.
- [ ] `async { }` sugar (низкий приоритет — поверх FiberHandle).
- [ ] `async_sleep_ms`, `select` двух каналов (минимум).

### 1.4 Честный бенчмарк
- [x] vs tokio: ЗАКРЫТО 2026-11-03 (bench_natvar/RESULTS.md). Echo
  1 conn × 10k msg: Datara 27,2k msg/s vs tokio 25,6k (+6%). 100k
  спящих: tokio быстрее ~80×/12× — задокументировано ЧТО и ПОЧЕМУ
  (OS-стеки vs ленивые future, round-robin vs timer wheel). p99 при
  ms-часах не фейкуем. 10k одновременных соединений НЕ поддержаны
  (fd_set 64 / NATVAR_IO_MAX 256) — известное ограничение, путь
  фикса: chunked select / IOCP (1.5.3).

**Критерий N1**: echo-сервер на фиберах (100 conn / 1000 msg) зелёный на
Cranelift+JIT+LLVM; deadlock ловится; бенчмарк с артефактом.

## N2. Чекер: ноль известных дыр `[ ]` (1 сессия)

НОВЫЕ repro, найденные при построении bench_json (2026-11-03, все —
сеанс-локальные чек-пробы, каждая закрыта обходом в бенче):
- [x] Behavior-метод в цикле с List<Int>.get — ЗАКРЫТО 2026-11-05
  (root cause найден бисекцией M1–M56): lowering/mod.rs регистрировал
  каждый метод класса ПОД ГОЛЫМ ИМЕНЕМ в глобальной
  function_return_types; JsonValue.get(key: Str) -> Str типизировал
  list.get(i) как Str для ЛЮБОГО получателя → значение трактовалось
  как строковый указатель в циклах (мусорные суммы, strcmp-краши,
  AOT+JIT). Фикс: голые регистрации удалены (класс-скоуп ключи
  остались); fallback в expr_call только при неизвестном классе.
  Regression: tests/test_repro_json_types_loop_get.rs 2/2. Полный
  cargo test зелёный; bench_json снова на настоящих методах.
- [x] Module `const` в use'd-модуле = 0 — ЗАКРЫТО 2026-11-05 (root
  cause через `inspect clif`: глобалы = per-function stack slots, init
  писал только слот main; функция модуля читала свой нулевой слот).
  Фикс: иммутабельные Int-консты фолдятся в ident-read
  (const_global_values). Regression: test_module_const_crossfn 1/1
  (5000/1000). ОТКРЫТ #10b: mut-глобалы между функциями всё ещё
  изолированы (нужен общий data-slot путь).
- [ ] Module-level `mut` в use'd-модуле изолируется per-function
  (мутации не переживают вызов).
- [ ] Module-level `mut Str` глобал теряет значение (init ""→Int 0,
  значение теряется через return).
- [ ] Outcome.ok/err из module-level fn use'd-модуля — битый Outcome
  (is_success=false у ok(), error_msg=указатель).
- [ ] E0941-проверка не принимает guarded `if x != 0` для вычисляемых
  локалов (только require/literal — `require JERR_SCALE != 0` тоже не
  работает для module const).
- [ ] Index-set `m[k] = v` на босом литерале `{}` с Int-ключом идёт по
  Str-пути (strlen(int) → segfault, AOT+JIT); `insert()` и Int-литералы
  `{1: 10}` работают (test_map_int_keys). repro #8, 2026-11-03.

Каждая дыра — известный repro из сессии 1.5.1:
- [x] Bare-generic body field access — ЗАКРЫТО 2026-11-03:
  compile_func.rs GetField/SetField — когда val_to_class не имеет
  receiver, используется built-in Outcome/Maybe ABI (is_success/
  is_some→0, value→8, error_msg→16). Verified:
  List<Maybe<Int>>.get(0).value → 42, .is_some → 1 на Cranelift.
  NOTE: the proper long-term fix is propagating the element class
  through the List lowering (value_classes in compile_func.rs).
- [x] Method-return `T` — ЗАКРЫТО 2026-11-03: flow-sensitive generic
  inference работает. При push_back(42) на Deque<T> (Class или
  GenericInstance с TypeParam) чекер записывает
  symbol_types[var] = GenericInstance{name, args=[Int]} и
  var_element_types[var] = Int. При pop_back() TypeParam резолвится
  через существующую GenericInstance подстановку. Регресс:
  test_generic_inference (deque_new + push_back(42) + pop_back → 42).
- [x] Явные generic-аргументы на вызове — ЗАКРЫТО 2026-11-03:
  parser postfix при виде `< Capitalized > (` после lowercase идентификатора
  пропускает type args как декорацию (тип выводится из call args через
  flow-sensitive binding). test_generic_inference 2/2.
- [ ] Статический ctor в цепочке: `Outcome.ok(1).is_success` — MemberAccess
  на GenericInstance-результат.
- [ ] Регресс на каждый пункт + обновить «Ограничения» в мануале.
- [ ] Optimizer-честность: PolyhedralVectorize не должен выдавать
  «zero loop-carried deps» для циклов с зависимостями (gcd-кейс) —
  проверка зависимостей перед emit метаданных; trace врёт = баг.

## N3. Стандартная библиотека до «идеальных библиотек» `[ ]` (2 сессии)

Правило модуля: заголовок-дока + @test-сьют в stdlib_tests + parity на
3 тирах. Ноль «каркасов».

- [x] **stdlib.net + minihttp** — ЗАКРЫТО: examples/showcase/minihttp —
  socket_create/bind/listen/accept + fiber_net_recv (парковка фибера
  до данных) + path-routing + HTTP-ответ. Чекер 100% OK. Побочно:
  встроенный Outcome ABI в field offset resolvers (обоих бекендов).
- [x] **stdlib.io.csv** — ЗАКРЫТО: parse_csv_row/csv_row/parse_csv
  (codepoint-aware).
- [x] **stdlib.math** — ЗАКРЫТО: stats (mean/median/stddev/min/max,
  proof-carrying Float-деление), random (xorshift64*, NonZero span
  proof). Matrix — отложено (низкий приоритет).
- [x] **stdlib.testing** — ЗАКРЫТО: Tester behavior с assert_true/
  false/eq_int/eq_str, счётчики, отчёт. Inline-fmt внутри behavior-
  методов работает (урок: fmt{param} — inline, НЕ отдельные аргументы).
- [x] **stdlib.text** — ЗАКРЫТО 2026-11-03: text_split_n (max parts),
  text_trim_start/end (codepoint-aware), text_lines (skip empties),
  text_repeat, text_replace_all. Verified 100% OK.
- [x] **stdlib.collections**: ЗАКРЫТО 2026-11-03. hash-Map уже O(1) в
  рантайме (Robin Hood, LF 3/4, индекс с 16 записей) — добавлена
  поверхность итерации: map_keys / map_keys_int / map_values /
  map_values_str (вставка-порядок, снапшот → удаление при итерации
  безопасно; Cranelift+JIT+LLVM+WASM). Тесты test_map_iteration.rs 4/4
  (вкл. 100k записей), STDLIB.md обновлён. Найден repro #8: index-set
  `m[k]=v` на босом {} c Int-ключом идёт по Str-пути → segfault
  (insert()/литералы работают; JIT-хост тоже).
- [x] **stdlib.json**: ЗАКРЫТО 2026-11-03. Парсер-добивка: reader.dtr —
  рекурсивный спуск RFC 8259 (строки/эскейпы/строгие числа/литералы,
  O(1) через str_bytes). Bench vs C 10 МБ (bench_json/RESULTS.md):
  VALIDATE 171 ms vs C 14.0 (12.2×), EXTRACT 111 ms vs 23.5 (4.7×),
  побитово совпадает (124259 / 7720087411 / 1615367). Бенч ПОЙМАЛ и
  починил реальный баг рантайма: str_substring делал strlen на каждый
  вызов — O(n²), фикс через memchr дал 663× (73.5 s → 111 ms).
  6 repro-дыр языка записаны в N2.
- [ ] Каждое: пример в docs/STDLIB.md.

## N4. Тулинг до 8/10 `[ ]` (1 сессия)

- [ ] **Debug-профиль**: `forgen build --debug` — DWARF-инфа проверяется
  тестом (test_dwarf_debug расширить: lldb/cdb подхватывает линии);
  раздел «Отладка» в мануале.
- [ ] LSP × 1.5: completions fiber_*/chan-методов, hover для `is`-нарровинга,
  E-ANY-001 в publishDiagnostics (test_lsp_advanced).
- [x] watch SLA-тест — ЗАКРЫТО 2026-11-03: test_watch_sla.rs —
  инкрементальный check после модификации < 200 мс.
- [ ] fmt-идемпотентность: fmt(fmt(x))==fmt(x) на examples+stdlib — CI-джоба.
- [ ] dpm init интерактивный (имя/версия/шаблон hello) + сценарные ошибки
  с подсказками.

## N5. Экосистема-витрина `[ ]` (1–2 сессии)

- [ ] **3 demo-проекта** (examples/showcase/, каждый с тестом):
  1. `wordcount` — CLI: чтение файлов, Map-агрегация, сортировка (Level 1-2)
  2. `minihttp` — веб-сервер на фиберах + SSR (Level 2, требует N1.1)
  3. `snake` — консольная игра: цикл, ввод, рандом (Level 1)
- [x] **WASM playground** — ЗАКРЫТО: docs/playground/ — index.html + 3 демо (fib/sieve/matmul) + build.sh.
- [ ] **Онлайн-реестр Spark'ов**: GitHub Pages (pages.yml есть) —
  index.json + подписанные пакеты + `dpm add <url>`.
- [ ] bridge_smoke.py: расширить до c/js/rust реальных прогонов.

## N6. Доки до 8/10 `[ ]` (1 сессия)

- [ ] docs-гейт РАСШИРИТЬ: ВСЕ секции мануала — сниппеты компилируются
  (сейчас только новые). Section-by-section verification.
- [x] Language Tour: docs/tour/language_tour_1_5.md — 1.5-фичи с верифицированными примерами (TSU/fibers/channels/MT/IO/atomics/Outcome/collections/testing/stats/toolchain).
- [x] README — ЗАКРЫТО 2026-11-03: stdlib features (testing/stats/random/csv/collections), winget PR status.
- [x] CONTRIBUTING: чеклист из реального гейта + архитектур-гайд +
  invariants (parity/field-order/Outcome-ABI/size-gate) — ЗАКРЫТО.
- [x] issue-шаблоны (bug/feature/stdlib_module) + публичная roadmap (PLAN_V152.md).

## N7. Дистрибуция `[!]` (блокируется ТОЛЬКО токенами владельца)

- [x] winget PR #467946 — ждёт CLA + ревью.
- [x] Release v1.5.1 — 17 артефактов, CI success.
- [ ] crates.io (cargo publish), npm, PyPI — токены от владельца.
- [ ] Homebrew tap (datara-lang/homebrew-tap, форма из tar+SHA256SUMS).
- [ ] Scoop bucket, Chocolatey, VS Code Marketplace.
- [ ] tools/release_checklist.py — «релиз одной командой».

## Порядок (сессии)

1. N1.1–1.3 (IO-поллер + атомики + async) — САМОЕ ВАЖНОЕ
2. N2 (чекер-дыры) + N0 (гигиена)
3. N3 (stdlib: net/math/testing/json)
4. N4 (тулинг) + N6 (docs-гейт × все секции)
5. N5 (витрина: демо/playground/Pages)
6. Релиз v1.5.2: bump + CHANGELOG_V152 + тег + release_checklist

## DoD «8/10»

1. Гейт: 0 failed, fmt/clippy 0, size-gate, parity, docs-гейт ВСЕ секции.
2. Echo-сервер на фиберах 100/1000 на 3 тирах + deadlock-детектор.
3. Ноль известных дыр чекера (список N2 закрыт с тестами).
4. stdlib.net/math/testing — с сьютами; json bench vs C с артефактом.
5. 3 demo-проекта + playground + Pages-реестр.
6. Релиз v1.5.2 во всех доступных реестрах (остальное — токены).
7. Единственное «не 10/10» = community — следующий этап после кода.
