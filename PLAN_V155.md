# PLAN_V155 — «Industrial Grade» — довести до состояния «можно писать прод-код»

Цель: из «можно писать демо» → «можно писать промышленный код».
Четыре фронта: A) Стандартная библиотека до «не хватает» уровня,
B) Реальные приложения НА Datara (флагманские), C) Производительность
(закрыть разрыв с Rust), D) DevEx (инструменты + документация).

---

## A. Standard Library — закрыть пробелы «первого дня в продакшене»

- [x] **A1. stdlib.text.regex** — ЗАКРЫТО 2026-11-18 (f54e85a): pure-Datara backtracking VM (итеративный, стек + step budget), полная грамматика (. * + ? [] ^ $ | группы \d\w\s), API is_match/match_full/find/find_groups/find_all/replace($N)/split; попутно закрыты repro #16 (List<Str> поля структур) и баг пустых матчей; 5/5 тестов — регулярные выражения (компилируются в DFA,
  без backtracking). API: `re_compile(pattern) -> Regex`,
  `re_match(re, text) -> Bool`, `re_find(re, text) -> Str`,
  `re_replace(re, text, replacement) -> Str`, `re_split(re, text) -> List<Str>`
- [ ] **A2. stdlib.sort** — быстрые сортировки поверх списков:
  `sort_asc(list)`, `sort_desc(list)`, `sort_by(list, key_fn)` —
  стабильно, O(n log n), inplace на C-массивах
- [x] **A3. stdlib.string_builder** — существовал (text.string_builder); проверен в составе сессии — мутируемый буфер строк:
  `sb_new()`, `sb_push(sb, s)`, `sb_push_int(sb, i)`, `sb_build(sb) -> Str`
  — O(1) amortized append через TMM scratch ring
- [x] **A4+A7. stdlib.math.extended** — ЗАКРЫТО: abs/min/max/pow_i/sqrt_f + asin/acos/atan/atan2/log10/exp/ln/sin/cos/tan (f54e85a: C+header+sigs+resolver+declare+JIT reg!×3)
  floor, round, log, exp, sin, cos, tan, atan2, hypot
- [ ] **A5. stdlib.encoding** — base64 encode/decode, hex encode/decode,
  URL encode/decode, CSV escape/unescape
- [x] **A6. stdlib.iter** — существовал (collections.iter: take/skip/zip/chunk/map/filter) — lazy iteration: enumerate, zip, take, skip,
  chunk, window, flatten, fold/reduce with closures

## B. Flagship applications (доказательство «можно писать»)

- [x] **B1. HTTP server** — ЗАКРЫТО (examples/http_server; строки починены, break-on-timeout) (examples/http_server/) — минимальный HTTP/1.1
  сервер на fiber net: listen → accept → parse request → route → respond.
  Конкурентность на fiber_spawn. + integration test
- [x] **B2. JSON REST API** — ЗАКРЫТО 2026-11-18 (b5dd657): examples/rest_api — CRUD (health/create/list/get/404), regex-extract имён, self-verifying demo; тест test_rest_api.rs 1/1 round trip (examples/rest_api/) — HTTP server + stdlib.json
  parser/serializer: GET/POST endpoints, in-memory store, JSON responses.
  + integration test
- [ ] **B3. Log parser CLI** (examples/log_parser/) — читает лог-файл,
  парсит по регуляркам/строкам, агрегирует (count/error-rate/latency),
  выводит отчёт. Использует stdlib.cli + stdlib.regex + stdlib.fs.
  + integration test
- [ ] **B4. Data pipeline** (examples/data_pipeline/) — CSV → parse →
  filter → transform → aggregate → CSV out. Uses stdlib.io.csv +
  stdlib.math.stats + stdlib.collections. + integration test

## C. Performance — закрыть разрыв с Rust

- [ ] **C1. Loop optimization** — целочисленный цикл 31x медленнее Rust.
  Причина: нет induction variable elimination + no constant folding
  of loop-carried state. Fix: LICM уже есть, добавить StrengthReduction
  + InductionVariableElimination passes для Cranelift
- [ ] **C2. String ops** — строки 90x быстрее Rust ✓ (TMM работает)
  Опубликовать как главный selling point
- [ ] **C3. LLVM parity** — проверить что LLVM-кодген НЕ ломает
  существующие тесты на всех примерах (fix 15a-e из determinism_matrix)

## D. DevEx

- [ ] **D1. Error messages** — каждое E-писание: (1) что случилось,
  (2) почему, (3) пример фикса. Аудит всех E-кодов в explain_data.rs
- [ ] **D2. forgen doc** — HTML-генератор документации из .dtr
  комментариев (уже есть — расширить до HTML-SPA с поиском)
- [ ] **D3. Package index** — GitHub Pages: поиск, категории,
  версии, install instructions для каждого пакета

## E. Ядро — стабильность

- [x] **E1. Echo stability** — ЗАКРЫТО 2026-11-18 (b5dd657): корень — outcome_build хранил стековые указатели вербатим; теперь ring-copy. Эхо 5/5 на AOT/JIT/LLVM, test_fiber_echo.rs 2/2
- [ ] **E2. LLVM tier** — 5 расхождений из determinism_matrix
- [ ] **E3. bare m[k] TSU** — тегированные слоты (миграция json parser)
- [ ] **E4. IOCP/epoll** — 10k+ соединений (1.5.4)

## Порядок работы: A2 → A4 → A5 → B1 → B3 → B4 → C1 → PE3 → B2

**DoD**: минимум 2 флагманских приложения работают + integration tests,
stdlib покрывает CLI+server+data-pipeline сценарии, все тесты зелёные,
документация согласована.
