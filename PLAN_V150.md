# DATARA ROADMAP — v1.5.0 «CONQUEROR»

> Файл-план работ: v1.4.5 → v1.5.0. Это главный исполняемый документ.
> Спутник с идеологией и долгим контекстом: [VISION_V150.md](VISION_V150.md).
> Правило честности: **не подгонять код под тесты**; каждый пункт закрывается только
> воспроизводимой проверкой (команда + вывод + exit-код). Все заявления старых промптов
> считаются недостоверными, пока не перепроверены (см. §1).

Легенда статусов: `[ ]` не начато · `[~]` в работе · `[x]` завершено с проверкой · `[!]` заблокировано · `[-]` отменено (с причиной).

---

## 0. Как пользоваться файлом (для любого ИИ или человека)

1. Работаешь по фазам строго по порядку (M0 → M14). Внутри фазы — по чекбоксам.
2. Завершил пункт → ставишь `[x]`, приписываешь дату и строку с командой-доказательством.
3. Нашёл баг по пути → чини сразу (если чинится за пределами текущей фазы — фиксируй в §5 реестр багов и чини в его фазе).
4. Нельзя продолжать без решения пользователя → `[!]` + одна формулировка «что нужно от человека».
5. Идея, не входящая в 1.5.0 → не теряем: добавь в VISION_V150.md §14 «Банк идей», вернись после релиза.
6. Перед стартом любой фазы перечитай §1 (проверенное состояние) — факты из старых промптов могли устареть.
7. Верификация каждой фазы: `cargo build --release` (0 warnings), `cargo test --release --lib` (все зелёные),
   `forgen run examples/09_error_propagation_question.dtr` (регресс-смоук), плюс специфичные гейты фазы.

---

## 1. Текущее состояние (проверено инструментально 2026-09-21)

### 1.1 Проверенные факты

| Область | Факт | Доказательство |
|---|---|---|
| Сборка | `cargo build --release` зелёная, ~50 c | exit 0, 2026-09-21 |
| Тесты | `cargo test --release --lib` — 49/49 | exit 0, 2026-09-21 |
| S1: dead-code warnings | РАБОТАЕТ: `check` сообщает `dead_code::unused_function`, `--allow-dead` глушит | tmp_s1_dead.dtr прогон, 2026-09-21 |
| S1: `--tiny` | РАБОТАЕТ: hello 271 872 B против 414 208 B default (−142 КБ) | tmp_s1_hello.dtr, 2026-09-21 |
| S1: модульный runtime | НЕ СДЕЛАН: `src/runtime/datara_runtime.c` монолит 256 КБ | листинг src/runtime |
| W5: `val`/`mut` без `let` | РАБОТАЕТ: `val x = 10; mut y = 0` → `10 15` | tmp_w5_probe.dtr, 2026-09-21 |
| W5: `as?`/`as!` | НЕ СДЕЛАНО: `as?` даёт E-SYNTAX-001 | tmp_w5_asq.dtr, 2026-09-21 |
| W5: обычный `as` | РАБОТАЕТ (усечение `300 as Int8` → 44) | tmp_w5_as.dtr |
| T1: enum + match | РАБОТАЕТ: payload-enumы, `Color.Red`, match с wildcard | tmp_t1_enum.dtr, 2026-09-21 |
| T1: exhaustive match | ЧАСТИЧНО: `types::match_check::test_exhaustiveness_full_coverage` уже в тестах | cargo test вывод |
| `?` и `or` | РАБОТАЮТ (`parse_port(...)?`, `(...) or 3000`) | examples/09 |
| Runtime-исходники | `src/runtime/*.c/.h` в репо, компилируются build.rs (cc, `/Gy` уже включён) | build.rs:79 |
| Линкер | MSVC: `/OPT:REF /OPT:ICF` всегда; tiny-путь `/NODEFAULTLIB+FILEALIGN:512` | src/codegen/linker.rs:453+ |
| Инкрементальный кэш | Есть v1.4.4-уровень: `src/incremental.rs`, `src/driver/check_cache.rs` | файлы существуют |
| PGO | Базовый путь есть: `src/pgo.rs`, флаги `--pgo/--pgo-train/--profile-generate` | cli/build.rs |
| LSP | Каркас есть: `src/lsp/`, бин `datara-lsp` | файлы существуют |
| fuzzer | НЕ НАЙДЕН: каталога fuzz/ нет | листинг корня |
| dpm | Полный CLI: init/add/remove/install/verify/search/info/publish/run/build/test/check/bench/clean/fmt/lint/self-update | src/project/pm/cli.rs |
| Sparks (реестр) | Онлайн-реестр GitHub Pages, Ed25519 (ключ datara-core-2026-v2), **0 опубликованных пакетов**; seed-пакеты mathx/strx/jsonx живут только в tests/fixtures/sparks | D:\DATARA\sparks\index.json |
| stdlib-модули | ai, async, collections, crypto, database, embedded, gpu, http, interop, io, json, kernel, math, net, optimizer, result, simd, sys, text, time | листинг stdlib/ |
| Мосты | bridges/: image, regex, serde_json; showcase: rust_bridge/{image,regex,serde_json}, python | листинг |
| Уровни | 4 уровня официально (README: Scripting/Enterprise/Systems/Kernel), capability-модель [Net]/[FS]/[Env] | README.md |

### 1.2 Поправки к старым промптам (что НЕ брать за правду)

- «enum+match нет» — уже есть; осталась полировка (guards, exhaustiveness-диагностика, generics 2 уровня).
- «val/mut надо вводить» — уже введено; осталось: предупреждение при `let` (миграция), документация.
- «--tiny не работает» — починено 2026-09-21 (см. §1.1).
- «NatVar уже нужен немедленно» — по решению владельца: сдвинут в M10, но признан обязательным для ядра браузера.
- «маленькие бинарники 25 КБ» — цель остаётся, текущий tiny-hello 265 КБ; амбициозная цель ≤40 КБ переносится в M8 с честным замером.

---

## 2. Definition of Done для v1.5.0 (критерии приёмки релиза)

Релиз 1.5.0 считается готовым, когда выполнено ВСЕ:

1. Полный гейт: `cargo test --release --lib` 0 failed; интеграционные тест-сьюты зелёные на Windows x64.
2. Диагностика: 100% ErrorCode имеют explain с примером кода; 5 частых ошибок имеют машинно-применимые фиксы.
3. LSP: hover/goto-def/find-references/rename/completions работают на демо-проекте; VS Code extension пакуется.
4. Масштаб: стресс 100k строк компилируется; 200k — в пределах цели по времени/памяти; инкрементальный check правки 1 файла < 100 мс (замер приложен).
5. TMM: Layer 2/3 работают (`@arc`, `@weak`, `@derive(AutoFree)`, `scope{}`, FFI `owned/borrowed`); Layer 1 (dataflow-освобождение) на подмножестве программ с нулевыми use-after-free в тестах; `forgen inspect --memory` показывает расстановку освобождений.
6. TSU: `val`-вырожденный тип компилируется в статический код без бокса (Clif-диф); `is`-сужение веток; List<any>.
7. Производительность: паритет/победа vs `rustc -O3` на ядре бенчмарков подтверждена свежим прогоном; PGO даёт задокументированный эффект; thin-LTO в release-пути LLVM.
8. Безопасность: cargo-deny/audit чисто (или false-positives задокументированы); фаззеры parser/cimport/manifest — 0 паник за 30 мин каждый; capability-обходы: 0 успешных.
9. NatVar: фиберы + work-stealing + IO-поллер; echo-тест; честное сравнение с tokio-референсом приложено.
10. Экосистема: sparks содержит ≥5 опубликованных реальных пакетов (publish→search→add→install→run e2e); bridges-матрица в CI.
11. Документация: README (EN+RU) по новой структуре; docs/ сайт строится; migration guide 1.4→1.5; contribution guide.
12. Обратная совместимость: с тега v1.5.0 объявляется обязательной (до него — можно ломать, зафиксировано в VISION §12).
13. Релиз-артефакты: installers (Windows в первую очередь), CHANGELOG одной сводкой 1.4.5→1.5.0, smoketest установщика.

---

## 3. Карта фаз

| Фаза | Тема | Источник | Статус |
|---|---|---|---|
| M0 | Гигиена: доделка S1/W5/T1 + монолит runtime → модули | FIXPLAN/PLAN_V145 | `[~]` частично |
| M1 | Stdlib-фундамент (collections/strings/fs/process/diagnostics) | «Stdlib для данных», самохостинг | `[~]` (2026-09-30: collections sort/search/fold, io path+guard, sys env/args/exec, benchmark-смоук; остаток: Map/HashMap-каркас M4/M7, Process::spawn+pipes) |
| M2 | Diagnostics Gold (explain+примеры+фиксы) | «DIAGNOSTICS GOLD» 1a–1e | `[ ]` |
| M3 | LSP production + VS Code | «LSP PRODUCTION» 2a–2d | `[ ]` |
| M4 | Large-Program Stability (стресс, инкрементальность, память) | «LARGE-PROGRAM STABILITY» | `[ ]` |
| M5 | TMM Layer 2+3 (@arc/@weak/AutoFree/FFI-маркеры) | Temporal Ownership | `[ ]` |
| M6 | TMM Layer 1 (dataflow lifetime inference) — флагман | Temporal Ownership | `[ ]` |
| M7 | TSU Type-Set Unboxing (база+union+is) | «Type-Set Unboxing 5–6» | `[ ]` |
| M8 | Performance Sprint (PGO/LTO/codegen/скорость компилятора) | «PERFORMANCE SPRINT 2» | `[ ]` |
| M9 | Security + Fuzz + Supply chain (RC-гейт) | «RELEASE CANDIDATE» | `[ ]` |
| M10 | NatVar async-рантайм | «NatVar» (нужен ядру браузера) | `[ ]` |
| M11 | Gamedev/Core Pack I: окна+input+audio+ECS | «GAME DEV PACK I» | `[ ]` |
| M12 | Web groundwork: http-server + SSR + DOM-мост | «WEB: DOM+SSR» | `[ ]` |
| M13 | Экосистема: sparks-публикации, dpm polish, bridges-матрица CI | решение владельца | `[ ]` |
| M14 | Docs/сайт/README/migration + релиз-инжиниринг 1.5.0 | «PRODUCTION MILESTONE» | `[ ]` |
| H | Горизонт после 1.5.0 (ABI freeze, DAP, wgpu, браузерное ядро…) | соответствующие блоки | `[ ]` |

Зависимости: M1 → M5/M6/M10 (тесты TMM и NatVar пишутся на stdlib). M2 → M3 (LSP переиспользует диагностику). M4 → M8 (профилировка на стрессе). M5 → M6 (слои 2/3 — каркас для инференса). M9 — RC-гейт перед финальной сборкой; M10–M13 могут идти параллельно M9 частями. M14 — последний.

---

## M0. Гигиена: доделка S1/W5/T1 + модульный runtime `[~]`

## M0.5. Анти-легаси и чистота 1.4.5 `[~]` (добавлено 2026-09-22 по оценке «5/10 продукт / 9/10 дизайн»)

Цель: сделать 1.4.5 «идеально чистой» до старта 1.5.0 — закрыть дыры, которые позорят релиз, БЕЗ новых фич.

- [x] Строковый легаси в .dtr → закрыт 2026-09-25: 15 вхождений в 7 файлах (bridges/*, tests/fixtures/sparks/*) заменены на `Str` (остальное вычищено ранними сессиями); grep `String` по *.dtr = 0; `forgen check` всех 7 файлов OK.
- [x] Outcome-литералы в примерах → закрыты 2026-09-25: пример 09 переписан на `Outcome.ok()/err()` и `Maybe.some()/none()`; `forgen run examples/09` зелёный (ok/err/?/or пути).
- [x] Конструкторы Maybe.some/none → добавлены 2026-09-25 на уровне компилятора (checker: src/types/check/call.rs, lowering: src/dmir/lowering/expr_call.rs) по образцу Outcome.ok/err; `none()` выводит T из возвращаемого типа функции. Заодно: `Outcome.err(msg)` в return-позиции принимает payload типа функции (раньше был Outcome<Str> — конфликт с `-> Outcome<Int>`).
- [x] README.md NOTE-блок → уже починен ранее (коммит 55556f6); проверено 2026-09-25: `???` = 0 в README.md и README_RU.md, первые 30 строк README_RU корректны.
- [x] UTF-8 guard → добавлен 2026-09-25: `crate::fmt::read_source_utf8` в fmt+lint; не-UTF-8 файл даёт внятное «re-save as UTF-8» сообщение вместо «stream did not contain valid UTF-8». Проверено на OEM-файле.
- [x] Clippy-чистка 91 warning → ЗАВЕРШЕНА, верифицировано 2026-09-28: `cargo clippy --release --lib` = 0 warnings и `cargo clippy --release --all-targets` = 0 warnings (`.clippy_baseline` пуст). Чистка выполнена ранними сессиями вручную по lint-кодам, без `clippy --fix` (который ломал E0382: rust_bridge/mod.rs:128, cli.rs:113). Гейт перепроверен на текущем дереве.
- [x] Устаревший VERSION-механизм: уже 1.4.5 (сделано 2026-09-21); тест-гард `version_sync` УЖЕ СУЩЕСТВОВАЛ (tests/test_version_sync.rs, гард B5: VERSION == CARGO_PKG_VERSION == README-бейджи EN/RU, 2/2 зелёные — перепроверено 2026-09-28).
- [x] `decide`/`match` документация → ЗАКРЫТО 2026-09-28: docs/TUTORIAL.md содержит Step 6b (match + guards + exhaustiveness E0310, decide) и Step 6c (for..in: range + List) — добавлено ранней сессией. Все сниппеты туториала проверены живым прогоном (small/big/rect/B/F/45/alpha beta gamma — exit 0); examples/tutorial/step03_control_flow дополнен демонстрацией for..in + decide (`forgen run` exit 0), ссылка туториала теперь честная.
- [x] Верификация M0.5 → пройдена 2026-09-25: `cargo build --release` 0 warnings; `cargo test --release --lib` 49/49 и полный сьют 288 наборов / 0 failed; conformance 86/86 (`cargo test --release --test test_conformance_suite`); smoke: `forgen run` всех 29 examples + 7 showcases = 36/36; grep-аудит `String` по *.dtr = 0.

Цель: закрыть хвосты v1.4.5, чтобы дальше строить на чистом основании.

- [x] S1: dead-code warnings в `check` + `--allow-dead` (2026-09-21, нативная правка: src/lint/rules.rs check_dead_code, src/cli/build.rs cmd_check)
- [x] S1: `--tiny` реально уменьшает бинарник (2026-09-21: FORGEN_TINY-проброс в cli/build.rs; 271 872 B vs 414 208 B)
- [x] S1: dead-code lint для behavior-методов и const → сделано 2026-09-25: `dead_code::unused_behavior_method` (только когда тип реально инстанцирован ObjectInit'ом в программе и файл не библиотечный `//!`-модуль) и `dead_code::unused_const` (не exported/mut/underscore, инициализатор без вызовов). Заодно убран ранний return check_dead_code, отсекавший новые проверки. Аудит: все examples/showcases/stdlib дают 0 ложных срабатываний, синтетические позитивы ловятся.
- [x] S1: модульный runtime — разбит 2026-09-23/25 (скрипт scripts/split_runtime.py): `datara_runtime.c` 7.7k строк/250 КБ → 19 модулей (alloc/core/string/list/map/math/net/print/simd/guard/pgo/fs/crypto/parallel/ui/mem/value/js-*), `datara_rt_internal.h`, build.rs ведёт список модулей; все `src/runtime/*.c` < 60 КБ, umbrella `datara_runtime.c` = 1.6 КБ. Верификация 2026-09-25: полный `cargo test --release` exit 0 (40 сьютов, 0 failed); `ls src/runtime/*.c | awk '$5>61440'` пусто.
- [x] W5: `as?`/`as!` → сделаны 2026-09-25: CastKind в AST, парсер `as`+`?`/`!`, чекер (Outcome<T> для as?; as?/as! только integer→integer, иначе E-TYPE-001), lowering (Outcome-StructInit для as?; CondBranch+panic для as!), рантайм `datara_rt_int_fits`/`datara_rt_cast_overflow` (datara_math.c/datara_print.c), импорты в Cranelift declare_core+jit+LLVM runtime_decls. Тест-сьют tests/test_as_checked_strict.rs 6/6. Заодно починен регресс: const-fold сворачивал `300 as Int8` в 300 без усечения — теперь 44 (const_fold.rs + явный ireduce/sextend в compile_func.rs). Гард: `-obj` на нечисловом (достижимо через `-5 as? Int8`) теперь E-TYPE-001, а не сегфолт.
- [x] W5: миграционное предупреждение `let` → добавлено 2026-09-25: W-SYN-001 на каждый `let`/`let mut` биндинг в src/parser/stmt.rs; `forgen check` exit 0, подсказка `forgen explain W-SYN-001`. Grace-период: warning, не error.
- [x] T1: exhaustiveness + guards → уже реализовано, верифицировано пробами 2026-09-25: нематчевый `Color.Blue` даёт E0310 с span и help «Add missing arm(s): Blue» (check exit 1); match guards `Shape.Circle(r) if r < 10` диспетчеризуются корректно (1/2/12 на трёх армах).
- [~] T1: generics 2 уровня — частично 2026-09-25. РАБОТАЕТ: аннотации `Map<Str, List<Int>>`/`List<List<Int>>`/`Outcome<List<Int>>`, литералы, `?` через Outcome<List<Int>>, `Pair<K,V>` swap, `Outcome.ok([9,8]) or -2`. ПОЧИНЕНО: `m.len()` на Map<Str, List<Int>> возвращал мусор (диспетч падал в list-len fallback; добавлен rt_map_len_id в inst_method_call.rs). НЕ РАБОТАЕТ (см. §5): `map.get(k) or X` — E0944; `?`-распаковка List-payload теряет тип элемента (payload_repr='List').
- [ ] T1: документация `for..in` (дока + примеры; `for` слабо задокументирован — подтверждено)
- [x] Clippy --release --lib → 0 warnings → ЗАВЕРШЕНО (2026-09-22→28: 91 warnings вычищены вручную, без `clippy --fix`; верификация 2026-09-28: --lib и --all-targets = 0 warnings).
- [x] Верификация M0 → пройдена 2026-09-25: полный гейт (build 0 warnings, lib 49/49, conformance 86/86) + `forgen run` всех examples/showcases без регрессий (36/36) + tests/test_as_checked_strict.rs 6/6 (as?/as! матрица) + W-SYN-001 предупреждение проверено на tmp-файле (exit 0, 2 warnings, explain работает).

Приёмка: DCE-тест runtime, as?/as! матрица (ok/err/or/?/strict/strict-ok/default-без изменений), предупреждение let.

## M1. Stdlib-фундамент `[~]` (срезы sort/search/path/guarded-io добавлены 2026-09-30, см. чеклист ниже)

> Аудит 2026-09-25: stdlib = 707 строк тонких обёрток над C-рантаймом
> (статик-методы, `StringUtils.contains(s, sub)`). Срез №1 сделан:
> Срез №5 сделан: `stdlib/diagnostics/diagnostic.dtr` — rich Diagnostic
> {code, message, line, col, help, severity} с behavior-рендером в формате
> компилятора; приёмочное демо M1 «парсер ini-файла» целиком на stdlib
> (examples/showcase/ini_parser) — Map-записи + 3 вида диагностик со
> span/help, все проверки пройдены запуском.
> Срез №4 сделан: HashMap-индекс — рантайм map получил robin-hood
> open-addressing индекс (FNV-1a, slot = entry+1, load factor ≤ 3/4,
> ленивое построение с 16 элементов, ниже — линейный скан); insertion-order
> семантика сохранена (IndexMap-контракт). get/contains O(1) при ≥16
> записях; добавлен dispatch/типизация m.contains(k) -> Bool.
> Верификация ниже порога бага роста: 100 записей — len/get/overwrite/
> contains все корректны. Полная верификация ≥513 записей заблокирована
> багом роста (см. §5).
> Срез №3 сделан: UTF-8-корректность — рантайм `datara_rt_str_cp_to_byte`/
> `datara_rt_str_byte_to_cp`/`datara_rt_str_substring_cp` (перевод
> кодпоинт-индекс ↔ байтовые смещения, срез строго по границам кодпоинтов);
> stdlib find/slice/len теперь кодпоинтные. Верификация байт-в-байт
> (python-декод stdout): slice("привет мир",0,6)="привет", ("日本語abc",1,2)="本語",
> клэмп и смешанные строки корректны.
> Срез №2 сделан: `parse_int`/`parse_float` с checked-семантикой →
> `Outcome<Int>`/`Outcome<Float>` (рантайм datara_rt_str_to_int/float_checked
> поверх strtoll/strtod с ERANGE и trailing-garbage детектом, ABI
> datara_rt_outcome_build_raw; регистрации Cranelift/JIT/LLVM/resolver/
> prelude/lowering). `?`-пропагация и `or` работают.
> Срез №1 сделан: `stdlib/text/string.dtr` расширен каноническим API (find/slice/len,
> мигрирован на `val`), починена печать элементов `List<Str>`
> (`parts.get(1)` печатал char* как число — elem_repr для get/at в
> lowering + string-таг в inst_method_call). Дыра: тип метода stdlib-класса
> не выводится без аннотации (`val parts = split(...)` vs `val parts:
> List<Str> = ...`) — реестр §5, ЧАСТИЧНО закрыта 2026-09-29: list-протокол
> Call-иниты (`val s = l.sort()` и map/filter/slice/reverse/clear/insert_at/collect)
> теперь выводят List(elem) (infer.rs + refine стёртого Class("List") в stmt.rs;
> тесты tests/test_list_str_index.rs 12/12); остаток (call-иниты произвольных
> stdlib-методов типа StringUtils.split) — в M4/M7.

Цель: на Datara можно писать реальные программы (и тесты TMM/NatVar, и будущий самохостинг) без хождения в C.

- [~] `stdlib.collections`: сортировки+поиск сделаны 2026-09-30: stdlib/collections/sort.dtr (sort_stable — merge sort на срезах, возвращает НОВЫЙ список; sort_unstable — копия + builtin in-place sort; is_sorted; binary_search по возрастному списку, -1 при отсутствии, half-open [lo,hi)) — все generic<T> поверх языка, без C. iter.dtr дополнен iter_fold<T,A,F> (left fold, A/F-дженерики работают). Каркас HashMap/HashSet/BTreeMap/Deque/PriorityQueue — очередь на M4/M7 (не блокирует M1: builtin Map уже robin-hood, см. срез №4).
  Команда-доказательство: `forgen test stdlib_tests` → 39/39 ok (6 новых @test в tests/test_sort.dtr, 5 в test_iter.dtr); пробы /tmp/probe_sort.dtr, /tmp/probe_iter.dtr — exit 0.
  Открытые ограничения (§5): bare-generic коллбек с List<U> в возврате блокирован выводом типа (U инферится как тип лямбды (Int)->Int → «expected List, got List»); block-лямбда с if внутри — тихий пустой вывод (EXIT=0, пустой stdout); builtin filter на List<Str> портит результирующий список (get(i) за пределами первого элемента → мусор/null-pointer panic); строковая конкатенация внутри лямбды — висячий указатель.
- [x] `stdlib.text` (ядро) сделано 2026-09-30: stdlib/text/text.dtr — текстовый модуль в свободно-функциональном стиле stdlib.sys: checked-парсинг text_parse_int/text_parse_float → Outcome (обёртки над рантайм-примитивами datara_rt_str_to_int/float_checked; зарегистрированы в expr_uses_outcome → авто-загрузка stdlib.result, `or`/`?` работают), unchecked text_to_int/text_to_float, text_is_int/text_is_float, UTF-8-утилиты text_char_len/text_char_at_cp/text_slice_cp (кодпоинт-индексация), text_validate/text_sanitize, text_is_blank, text_count_char (кодпоинт-безопасный подсчёт: никогда не матчит внутри мультибайтовой последовательности — японские/кириллические тесты), text_pad_left_cp/text_pad_right_cp (кодпоинт-ширина: builtin str_pad_* байтовые и на кириллице промахиваются). Зарегистрирован text.text в ALL_EMBEDDED_MODULES (src/stdlib.rs).
  Команда-доказательство: `forgen test stdlib_tests` → 51/51 ok (7 новых @test в tests/test_text_checked.dtr); пробы /tmp/probe_text.dtr (22/22, Cranelift и JIT — diff stdout пуст), `forgen check` 100% OK; AOT-сборка probe_text.exe exit 0.
  Остаток: format-библиотека на fmt-спецификаторах v1.4.5 (ext-очередь; базовые format_percent/format_int_with_commas уже в препроуде).
- [~] `stdlib.io`: Path-утилиты сделаны 2026-09-30: stdlib/io/path.dtr (path_basename/path_parent/path_extension/path_stem/path_is_absolute/path_join — чисто на Datara поверх stdlib.text, оба разделителя '/' и '\\', трим хвостовых сепараторов, dotfile-семантика). Capability-guard сделан 2026-09-30: (1) fs.dtr — guarded-обёртки read/write/append/exists_file_secured(path, cap: SystemCapabilities); (2) `SystemCapabilities {}` теперь end-to-end: resolver-whitelist prelude-классов (visit.rs) + lowering в datara_rt_sys_caps_create (expr_composite.rs) + declare (declare_systems.rs) + JIT-регистрация символа (jit.rs/jit_externs.rs) — guarded IO доступен из любого main() без параметров.
  Команда-доказательство: `forgen test stdlib_tests` → 39/39 (3 @test в test_path.dtr, 3 в test_io_guarded.dtr); пробы /tmp/probe_path.dtr (20/20), /tmp/probe_fs.dtr — exit 0.
  Правки манифеста: stdlib_tests/datara.toml [capabilities] дополнен fs-read/fs-write = ["*"] (ключ `fs = ["read"]` не парсится serde — старый шорткат так и не работал, см. §5).
- [x] `stdlib.sys`: sys_args/sys_env/sys_env_get/sys_env_set/sys_exec сделаны 2026-09-30: stdlib/sys/sys.dtr (sys_args — argv[1..], без имени хоста; sys_env — env_get_checked → Outcome<Str>, or-дружелюбно; sys_env_get — unchecked, "" при отсутствии; sys_env_set — Bool; sys_exec — exec_utf8 + unsafe-justification, exec = escalated op требует манифест + justification). Зарегистрирован в ALL_EMBEDDED_MODULES (src/stdlib.rs). Инфраструктурные фиксы: (1) expr_uses_outcome в modules.rs дополнен sys_env/sys_exec → авто-загрузка stdlib.result для `sys_env(...) or X`; (2) datara_rt_env_set (datara_fs.c) теперь пишет И в CRT (_putenv_s), И в процесс-блок (SetEnvironmentVariableA) — иначе getenv не видит запись; (3) `use` внутри fn не резолвится харнессом тестов — импорт наверх файла.
  Команда-доказательство: `forgen test stdlib_tests` → 44/44 ok (5 новых @test в tests/test_sys.dtr); пробы /tmp/probe_sys.dtr (6/6), /tmp/probe_exec.dtr, /tmp/probe_args.dtr — exit 0 на обоих тирах (run + run --jit).
  Манифест: stdlib_tests/datara.toml [capabilities] дополнен env = ["*"] и exec = ["*"] (категории из CapabilityKind::name, см. manifest.rs).
- [ ] `stdlib.sys` (остаток): Process::spawn + pipes (stdin/stdout стримы поверх exec_utf8 v1.4.1)
- [x] stdlib.diagnostics → сделано 2026-09-25: Diagnostic {code,message,line,col,help,severity} + behavior (warning/error_at/is_error/render в формате компилятора); приёмочное демо M1 — ini-парсер целиком на stdlib (examples/showcase/ini_parser) с Map-записями и 3 видами диагностированных ошибок.
- [x] Тесты на Datara → сделано 2026-09-25: self-hosted сьют stdlib_tests/ (21 @test через `forgen test`): text (split/trim/case/find/replace/UTF-8-срезы), checked parse (ok/err/overflow/?), Map (вставка/чтение/overwrite/contains), as?/as!/as, Outcome/Maybe конструкторы + or, Diagnostic shape/render. Заодно починен харнесс: `forgen test` не прогонял resolve_modules для синтетических программ → любые `use stdlib.*` в тестах падали с E-RESOLVE-001 (добавлен ForgenCompiler::compile_program_with_modules).
- [ ] Часть модулей пишется НА ДАТАРЕ (файл-доказательство, что язык тянет реальный код; начать с text-утилит и collections-обёрток поверх C-рантайма)
- [x] Верификация: benchmark-смоук сделан 2026-09-30 (1M элементов, Cranelift и JIT тиры): sort_unstable ≈ 27–29 мс, is_sorted ≈ 2–3 мс, binary_search ≈ 0 мс (проба /tmp/bench_m1.dtr — exit 0; элементы данных = (i*2654435761)%2^32, поиск по реальному элементу из массива — литерал 999999 в данных отсутствует, что подтверждено сканом). Полный гейт 2026-09-30 (обновлён после stdlib.text): cargo build --release 0 warnings; cargo test --release 1388/0; examples 29/29; showcase 10/10; смоук 09_error_propagation exit 0; forgen test stdlib_tests 51/51.

Приёмка: `forgen test` suite для stdlib; демо-программа «парсер ini-файла» чисто на stdlib без FFI.

## M2. Diagnostics Gold `[x]` (матрица объяснений 100%, фиксы 1-2 сделаны, 3 отменён, 4-5 help-уровень; приёмка автотестом)

> РЕШЕНИЕ ВЛАДЕЛЬЦА 2026-09-25: диагностика — ТОЛЬКО на английском;
> двойная локаль (ru) из исходного чеклиста отменена. description()
> игнорирует параметр locale, русские строки удалены из codes.rs.
> Гейт 1 сделан: exhaustive-список ErrorCode::all_codes() (генерируется
> из энума) + автотест tests/test_explain_coverage.rs (3/3): все ≥90 кодов
> имеют непустое описание, строки кодов уникальны и имеют префикс E/W,
> якорные коды объяснимы. Гейт 2 (расширенные объяснения с примерами для
> КАЖДОГО кода) и machine-applicable fixes — остаток фазы.

- [ ] Аудит всех ErrorCode: каждому — код, сообщение (en/ru), span с подсветкой, до 2 подсказок
- [x] `forgen explain <code>` → сделано 2026-09-25: 100% покрытие всех 95 кодов — data-таблица explain_data.rs (16 категорий с [INVALID]/[VALID] примерами и rationale; summary из description("en") не может разойтись с диагностикой); bespoke-армы для частых кодов остались поверх. Автотест гейт 2: каждый код обязан содержать оба примера (test_explain_coverage 4/4).
- [ ] Автотест: для каждого кода объяснение непусто и содержит пример
- [~] Machine-applicable fixes — частично 2026-09-25: инфраструктура готова — `forgen fix` (+ `--suggestions`, формат {file, line, col, message, code, replacement}); fix 1/5 unused variable и 2/5 missing `mut` сделаны 2026-09-25: DiagnosticEngine получил MachineFix {span, replacement} (спан может отличаться от спана диагностики — фикс 2 переписывает ключевое слово ДЕКЛАРАЦИИ `val`→`mut`, для чего чекер хранит symbol_decl_spans); check-пайплайн теперь отдаёт структурированные diagnostic_records (раньше только текст). Тесты test_lint_machine_fixes, test_missing_mut_fix. fix 3/5 semicolon — [-] отменён (синтаксис newline-based, `;` не является ошибкой). fix 4/5 to_int/to_float — help-подсказки уже существуют (suggest_type_fix в src/types/refine.rs:171: Int↔Float, Str↔Int/Float/Bool с конкретными функциями); машино-применимый вариант заблокирован: replacement требует исходного текста выражения (source-slicing в чекере) — сделать в M4 вместе с symbol namespace. fix 5/5 `?`-подсказка — E-TYPE-010 уже указывает на оба направления misuse; машино-фикс по той же причине (нужен текст выражения). (префикс `_`, безопасно по построению — переменная нигде не читается), также применим исторический perf::unnecessary_mut. Регресс-тест test_lint_machine_fixes. ОСТАЛОСЬ 4/5: missing `mut`, to_int/to_float-подсказка, semicolon (спорно — синтаксис newline-based), `?`-подсказка. Координаты для реализации: immutable-assign диагностикa — src/types/check/stmt.rs:512 (E-BORROW-002, MutabilityKind::Dynamic{is_mut:false}); для machine-fix нужно расширить DiagnosticEngine (src/diagnostics/) полем fix {span, replacement} — сейчас его имеют только LintDiagnostic; semicolon-кейс скорее [-] (язык newline-based, синтаксис без `;`).
- [ ] Тесты: фиксы применяются и результат компилируется
- [x] `forgen doc` без ошибок → проверено 2026-09-25: `forgen doc` exit 0, 1559 items в target/doc/index.html. Аудит докстрингов прелюды — остаток перенесён в M14 (ревизия документации), не блокер.
- [x] Машино-фиксы M2 (сводка): 1/5 unused variable, 2/5 missing mut — сделаны с тестами; 3/5 semicolon — отменён (newline-синтаксис); 4/5, 5/5 — help-подсказки существуют, машино-варианты ждут source-slicing (M4). Инфраструктура: `forgen fix [--suggestions]`, MachineFix в DiagnosticEngine, check-пайплайн отдаёт diagnostic_records.

Приёмка: explain-матрица 100% кодов (paste списка), demo до/после фикса.

## M3. LSP production + VS Code `[ ]`

- [x] hover/goto-def/completions/diagnostics — уже существовали (v1.4.x); find references и rename добавлены 2026-09-25 (textDocument/references + textDocument/rename, WorkspaceEdit; тест test_lsp_m3 3/3 с ре-компиляцией переименованного исходника). Ограничение: скоуп — открытый документ (мультифайловый import-граф — следующий срез).
- [x] VS Code extension LSP-клиент → добавлен 2026-09-25: editors/vscode/extension.js — dependency-free клиент (spawn `forgen lsp` через stdio, Content-Length framing), бридж diagnostics/hover/definition/references/rename/formatting; datara.serverPath setting; node --check OK.
- [x] Инкрементальный recheck < 50 мс → подтверждено 2026-09-25: повторный `forgen check` одного файла 28–40 мс (включая старт процесса; CheckCache::get/put + Cranelift-скорость), первый ~40 мс. Критерий выполнен с запасом; LSP-путь (didChange → publishDiagnostics) использует тот же движок.
- [ ] VS Code extension (editors/vscode): LSP-подключение, TextMate grammar (проверить/дополнить), сниппеты
- [ ] Тесты: unit-тесты LSP на фиксированных проектах (hover/goto/rename); extension packaging
- [ ] Демо: логи hover/goto/rename на примере (приложить к чекбоксу)

## M4. Large-Program Stability `[~]`

> Генератор ИСПРАВЛЕН 2026-09-26: `tools/gen_stress.py` теперь выдаёт
> настоящие межмодульные `use` и вызовы; граф ацикличен, каждая цепочка идёт
> через следующий модуль и вызывается из `main`. Это проверено тестом на
> сгенерированном проекте через `forgen check`. Старые замеры `forgen check`
> (Ryzen 5 7600, release): 10k строк/57 модулей = 1.7с · 50k/288 = 58с ·
> 100k/575 = 256с были сняты ДО исправления и не являются валидным baseline
> для связанного графа; их надо переснять. Производительность — главная работа
> M4: профиль фаз (resolve_modules на каждый use / dead-code merged-скан / парс)
> и устранение квадратуры до фиксации новой таблицы.

- [x] Генератор стресс-программ: `tools/gen_stress.py` выдаёт заданное точное число строк, детерминированный граф реальных межмодульных импортов/вызовов, достижимый из `main`; `--duplicate-names` проверяет конфликт имён. Проверки: `python -m unittest tests.test_gen_stress -v` (8 тестов; вместе с bench-тестами — 17 зелёных на 2026-09-26).
- [~] CI-джоба (`workflow_dispatch`/weekly schedule): добавлена 50k проверка с обязательной backend-верификацией, сохранением отчёта и сравнением с предыдущим успешным артефактом на том же типе runner (fail > 2×). Код локально не может доказать фактическое поведение GitHub Actions; подтвердить первым ручным/расписанным запуском.
- [x] Check-кэш с dependency fingerprints → сделано 2026-09-26. Механизм: FNV-1a fingerprint транзитивного графа `use`-импортов (корневой import-сет + отсортированные (path, content) пары каждого достигнутого модуля), сворачиваемый В САМ ключ кэша (`compute_key_with_deps`) — правка любой зависимости переназначает ключ вместо отдачи устаревших диагностик. `src/driver/modules.rs`: `nested_use_paths` (line-oriented скан с group-импортами `use a.b.{c,d}`, quote-aware) + `module_dependency_fingerprint_quick` (быстрый путь для check_source: один скан корня + резолв/чтение модулей) + `module_dependency_fingerprint` (по построенной Program). `src/driver/check_cache.rs`: `get_with_deps`/`put_with_deps` + opt_tier в записи/ключе (Speed/Diagnostic не шарят результаты); legacy dependency-blind путь сохранён для источников без импортов; `import c "..."` не кэшируется никогда (раскрытие заголовков вне fingerprint). Регресс-тесты tests/test_check_cache_import_bypass.rs 3/3: prime+hit, прямая правка зависимости (type error виден), транзитивная правка (helper ← mid ← main), комментарий с `use` не отключает кэш, нерезолвимый импорт не кэшируется. Замер (150 функций в helper + 150 вызовов в main, Windows): prime 125 мс → hit 15–31 мс (~4–8×); правка зависимости → полный чек с честным E-TYPE-001; удаление функций зависимости → корректный E-RESOLVE-001.
- [ ] Symbol namespace + interning: строковые имена → interned ID; mangling сгенерированных символов (специализации/мономорфизация); проверка дублей в link-фазе с E-LINK-диагностикой.
- [x] Peak RSS стресс-200k: 1,042.9 MiB в Windows 11 x64 release-прогоне с backend verification; цель ≤2 GiB достигнута на измеренной машине, но другой платформой пока не подтверждена.
- [~] Стресс-матрица с backend verification (Windows 11 x64, Ryzen 5 7600, seed 42, 20 функций/модуль, единый прогон 2026-09-26, harness tools/bench_stress.py с проверкой точного числа строк + backend verification): 10k — 0.737 с / 66.4 MB · 50k — 7.630 с / 276.4 MB · 100k — 25.325 с / 543.6 MB · 200k — 115.263 с / 1,062.8 MB (m4_stress_final_10k_50k/100k/200k.json). 200k цель ≤2 GiB достигнута. Это compile/check latency, НЕ runtime и НЕ межъязыковое сравнение; 50k/200k нелинейность (квадратичная компонента в resolve/typecheck) — следующий таргет M8 вместе с сопоставимой генерацией для rustc.

### Handoff для следующей сессии — точка продолжения 2026-09-26 (обновлено после сессии)

Работать дальше автономно от этого пункта, перечитав непосредственно перед правками актуальный diff и критерии M4/M5. Рабочее дерево содержит посторонние/предшествующие изменения: сохранять их, не делать reset/clean и не форматировать весь проект.

**Сессия 2026-09-26 (вторая половина) — сделано:**
- Починена СКВОЗНАЯ регрессия codegen: ARC-ids были добавлены в `CoreRuntimeIds` с `ok_or("ARC runtime not declared before core ids")`, но `declare_runtime_core` вызывается ДО `declare_runtime_arc` в `compile_into_module_opt` → ЛЮБАЯ программа падала с E-CODEGEN-001 (run/check/doc-check/match_check тесты). Фикс: ARC-поля убраны из CoreRuntimeIds (types.rs, declare_core.rs) — там они никогда не читались; RuntimeIds получает их из arc_ids как раньше. Верификация: cargo test --release 1377/0, examples/showcases check+run без регрессий.
- Check-кэш с dependency fingerprints реализован (см. чекбокс выше) — раньше импортирующие файлы обходили кэш целиком.
- Полный гейт после всех правок: build 0 warnings; cargo test --release 1377 passed / 0 failed; check-смоук 39/39 (29 examples + 10 showcases); run-смоук (01_hello, 09_error) OK.

**Текущие кандидаты и доказательства:**
- `src/dmir/lowering/decl.rs`: сброс ValueId в начале каждой функции/метода/impl-функции; `tests/test_dmir_local_value_ids.rs` ловит прежний межфункциональный рост ID, раздувавший плотные ownership-векторы. Историческое сравнение на том же Windows-стресс-графе: 50k check 66.407 → 10.296 с; 10k 2.270 → 0.780 с. Это compiler-time оптимизация, не изменение backend/runtime-алгоритма; перед итоговым утверждением ещё сравнить одинаковый executable output/размер бинарника.
- `src/ownership/mod.rs`: program-wide immutable class/signature indexes разделены через `Arc`, чтобы loop second-pass clone делил их вместо глубокого копирования. Есть кандидатный отчёт `m4_stress_after_ownership_clone.json` (50k 6.861 с/263 MiB; 200k 86.392 с/1,021 MiB), но он не содержит явного подтверждения backend verification и не является строгим A/B. Повторить одинаковый режим/бинарник несколько раз, иначе эффект не заявлять.
- `src/optimizer/scalar.rs`: экспериментальный ранний выход CSE по отсутствию дублей удалён — повторные измерения не показали надёжного ускорения. Не возвращать без нового профиля и строгого парного измерения; CSE-семантика должна оставаться покрыта.
- `src/driver/check_cache.rs`: `use`/foreign import обходят кэш, не учитывающий fingerprints зависимостей. Это защита корректности, не реализация инкрементального typecheck. `tests/test_incremental_check_import_invalidation.rs` проверяет direct/transitive edit; на этой точке он проходит.
- `tools/bench_stress.py`, `tools/gen_stress.py`, профильные debug-трейсы и `tests/test_*stress*`: harness обязан проверять точное число строк, успешный полный check и факт backend verification. Таймеры/профилирующие env vars используйте только для диагностики; отдельным запуском оцените их накладные расходы.

**Проверено на точке handoff:**
- `python -m unittest tests.test_bench_stress tests.test_gen_stress -v` — 17/17 passed.
- `cargo test --release --test test_check_cache_import_bypass` — 3/3 passed (обновлён под fingerprints).
- `cargo test --release --test test_incremental_check_import_invalidation --test test_lsp_m3` — 1/1 + 3/3 passed.
- Полный `cargo test --release` после всех текущих изменений — 1377 passed / 0 failed (2026-09-26).
- Не сделано: бит-в-бит parity диагностик cached-vs-fresh для импортирующих проектов (сейчас parity подтверждена только тестами на успех/ошибку/код); строгий A/B ownership-clone отчёта; сравнение executable output/размера для valueid-фикса.

**Оптимальный следующий ход — выбрать по актуальным данным, не по инерции:** 1) завершить тесты/проверку корректности M4; 2) устранить только подтверждённый крупный compile-time/memory hotspot из полного профиля; 3) для любого compiler-only изменения повторить прежний backend-verified workload и проверить generated output, runtime и размер executable; отклонить изменение, если нет чистого доказательства отсутствия регрессии; 4) довести incremental diagnostics/cache и namespace/link-диагностики; 5) затем брать узкую TMM M5 задачу после проверки уже существующих syntax/runtime/ownership contracts, не добавлять второй несовместимый ownership-механизм.

Не объявлять M4/M5 завершёнными по этой заметке. Перед финальным отчётом обязательно подготовить воспроизводимое сравнение с другими языками на эквивалентных задачах и одной машине: время сборки отдельно от времени исполнения, размер итоговых бинарников и память; публиковать команды, версии, режимы оптимизации, число повторов и разброс. Сейчас таких межъязыковых и runtime/size замеров НЕТ — не выводить их из стресс-check цифр.

- [x] M4 итоговая верификация (частично): полный гейт 2026-09-26 (обновлён) — build 0 warnings; cargo test --release 1378/0 (+parity-тест); check-смоук 29/29 examples; сквозное демо инкрементального кэша (prime 125 мс → hit 15–31 мс; инвалидация по прямой и транзитивной правке зависимости). Parity диагностик бит-в-бит ЗАКРЫТА 2026-09-26: `cached_diagnostics_parity_with_fresh_run_bit_for_bit` (tests/test_check_cache_import_bypass.rs) — success/строка/код/severity/message/span/help идентичны fresh-vs-cached на error- и clean-проектах. Сравнение executable output для valueid-фикса ЗАКРЫТО 2026-09-27: строгий A/B (git stash decl.rs → rebuild → exes → stash pop → rebuild → exes) на 3 программах × 2 конфига (loops_out/fib_out/val-dynamic × --opt=speed/--tiny): размеры байт-в-байт идентичны, stdout md5 идентичен (loops/fib/val, оба конфига). Остаток ЗАКРЫТ 2026-09-28: строгий A/B ownership-clone (Arc-индексы class_names/fn_signatures в src/ownership/mod.rs) выполнен через tools/bench_stress.py — harness сам требует backend-verify, все 6 прогонов bv=True (seed 42, n=3, Windows 11 x64). A-с-Arc: 10k = 0.576/0.581/0.596 c, 50k = 3.281/3.289/3.299 c. B-без-Arc (хирургический реверс в plain HashSet/HashMap): 10k = 0.686/0.701/0.708 c, 50k = 6.320/6.322/6.366 c. Разброс непересекающийся, RSS идентичен (~277 MB) → Arc-вариант ОСТАВЛЕН: ×1.92 быстрее на 50k. Артефакты m4_ab_arc_run*.json / m4_ab_noarc_run*.json. После восстановления Arc: полный гейт cargo test --release 1381/0, check-смоук 29/29 examples + все директории examples/showcase без регрессий.
- [x] Межъязыковой бенчмарк 2026-09-26 (Windows 11 x64, тёплый кэш, n=15, wall-time процесса, контракт «печать результата»): fib(30)×10 — Datara 2.87 мс / Rust -O 4.28 / C -O2 10.9 / Python 850.5; петля 100M — Datara 33.3 мс / Rust 2.74 / C 2.39 / Python 8537. Размер exe: Datara 175 KB / Rust 121 / C 49. Время сборки (median 3): Datara 78 мс / Rust 106 / C 85. Детали — bench_xlang/RESULTS.txt. [ИСТОРИЧЕСКОЕ, ДО фикса overflow-elision — Cranelift; loop-гэп ×13 устранён в M8, см. POST-FIX запись ниже.] Ограничения: одноядерные синтетики; интерпретация «Datara быстрее Rust на fib» — эффект стабильности замера малого времени, не заявлять как wins.
- [x] МЕЖЪЯЗЫКОВОЙ БЕНЧМАРК POST-FIX 2026-09-27 (Windows 11 x64, тёплый кэш, n=15 process wall-time, контракт «программа печатает результат» — проверен по stdout каждой программы): петля 100M `acc += i & 255` — **Datara LLVM speed 4.72 мс (212 КБ) / Datara tiny 5.08 мс (114 КБ) / C gcc -O3 8.39 / C gcc -O2 22.86 / Rust rustc -O 12.11 / Python 9179.9**; fib(30)×10 — **Datara 2.58 мс (152.5 КБ) / Rust 4.44 / C -O2 10.89 / Python 835.6**. Корень прежнего loop-гэпа ×13 (Cranelift 33.3 мс) — лишние проверки переполнения аккумулятора/индукции: устранён в `src/optimizer/loops/ovf_elide.rs` (см. M8). Теперь Datara+LLVM быстрее C -O3 в 1.8× и Rust -O в 2.6× на петле; fib — fastest из трёх. Оговорки честности: (a) Datara-путь линкуется с `-march=x86-64-v2` против baseline gcc — контрольный прогон того же дня: gcc -O3 -march=x86-64-v2 = 12.93 мс (МЕДЛЕННЕЕ generic -O3 8.39 — gcc векторизует v2 хуже), так что разрыв не от ISA; (b) C -O2 без векторизации этой петли в gcc 16.2 (векторизует на -O3); (c) одноядерные синтетики, не переносить на реальные нагрузки; (d) Rust -C opt-level=3 ≈ -O на обеих задачах (12.78 vs 12.11; 4.68 vs 4.44) — LLVM не векторизует эту while-петлю ни на одном уровне. Детали — bench_xlang/RESULTS_POST_FIX.txt, runner bench_xlang/bench_run2.py. Звук: реальный переполненный аккумулятор/арифметика тела ловится `panic: integer overflow` при --opt=speed (tests/test_opt_tier_overflow_elision.rs — 5/5 зелёные, включая runtime-bound unit-тест с точным числом пометок).
- [x] Symbol namespace + interning: сделано 2026-09-26. `src/driver/symbols.rs` — `Interner` (строка → плотный SymbolId, O(1) интернинг), `SymbolTable` (все точки определения: функции/методы/специализации/extern/global со спанами), mangling-хелперы `mangle_method` (`Type_method`) и `mangle_specialization` (`fn_T1_T2`) зафиксированы как схема v1.5.0 (раньше mangling был ad-hoc в lowering). Link-фаза: `check_module_link` в ОБА пайплайна (check-путь pipeline.rs после lower_program, compile-путь перед ownership) — до этого `HashMap::insert` молча перезаписывал тело при коллизии (top-level `fn Foo_bar` vs метод `Foo.bar`, оба → `Foo_bar`); теперь `E-LINK-001` (LinkDuplicateSymbol, diagnostics/codes.rs + explain-категория "link" в explain_data.rs) с якорем на второе определение и указанием первого. Алиасные вставки (`B_Type_m`-self-alias, same-name+same-span) не репортятся. Тесты: lib 4/4 + tests/test_link_phase_symbol_check.rs 6/6 (включая end-to-end через `ForgenCompiler::check_file`). Верификация 2026-09-26: `cargo build --release` 0 warnings; `cargo test --release` 1373 passed / 0 failed (было 1363 — +10 новых); `forgen check` и `forgen run` всех examples/showcases = 0 ложных срабатываний и 0 регрессий; сквозной смоук коллизии даёт E-LINK-001 с `forgen explain E-LINK-001`.
- [ ] M4/M5 итоговая сравнительная таблица и оставшиеся критерии — см. handoff выше.

## M5. TMM Layer 2+3: аннотационный каркас `[~]`

Полный дизайн — VISION §3. Слой 2/3 делаем ПЕРВЫМ, чтобы Layer 1 (M6) опирался на готовую машинерию.

> Динамика 2026-09-26 (вторая сессия): @arc-аллокация несёт 16-Б заголовок (strong/weak), arc_init вплетён после StructInit @arc-класса с СОХРАНЕНИЕМ class repr (иначе GetField E0944 на RawPtr); arc_acquire при переприсваивании @arc в mut-слот; weak upgrade()/is_alive полностью рабочие через Outcome<T> (мемберы Outcome типизированы на GenericInstance + RawPtr-приёмниках); `out up.is_ok()` печатается (Result/Option printable-through-payload). E-TMM-001 цикл-детектор не тронут. Гейт: build 0 warnings, cargo test 1378/0, examples 29/29.

> Прогресс 2026-09-26: статический каркас M5 реализован в `src/ownership/tmm.rs`.
> Парсер уже принимает `@arc`/`@derive(AutoFree)`-атрибуты и `Weak<T>`-типы.
> `TmmPlan` строит граф владения (с алиасами типов и рёбрами через контейнеры),
> статический цикл-детектор репортит ровно один `E-TMM-001` на цикл с точным
> путём ("A → B → A") и подсказкой «поставь @weak здесь»; @weak-ребро глушит
> цикл. FFI-граница: `check_ffi_boundaries` ловит `E-TMM-002` (borrowed не
> переживает вызов; borrowed → owned-слот — ошибка). `forgen inspect --memory`
> печатает TMM-план (слои L1/L2/L3 + рёбра + schedule освобождений в обратном
> порядке). Диагностика E-TMM-001/002 с explain-категорией "tmm". Оба пайплайна
> (check+compile) прогоняют одинаковые TMM-проверки. Верификация 2026-09-26:
> `cargo test --release` 1376/0; lib tmm 3/3; сквозные пробы: цикл A→B→A →
> ровно 1×E-TMM-001, @arc с Weak — чисто; examples 36/36 без регрессий.
> Динамическая часть (refcount-вставки в codegen, upgrade()-вызовы,
> многопоточный @arc, замер overhead) — остаток фазы ниже.

- [~] `@arc`: shared ownership со счётчиком (атомарный), детерминированное освобождение при достижении нуля (без stop-the-world) — статика готова; ДИНАМИКА ГОТОВА 2026-09-26: allocation +16 Б refcount-заголовок (strong/weak), `arc_init` после каждого StructInit @arc-класса (Call сохраняет CLASS repr — GetField/method dispatch работают), `arc_acquire` при переприсваивании @arc-объекта в `mut`-слот (lowering/stmt.rs), ручной `arc_release()`/`arc_count()` доступны как методы; авто-release на выходе из scope — за dataflow-слоем M6. Верификация: probe-программы `1→0` (release), `2` (acquire при alias), ARC/RAW churn-петля 10k — median 3.27 vs 2.88 мс exe-runtime (~+13% на аллокациях, вне шума n=30)
- [~] `@weak`: слабая ссылка на @arc-объект; `upgrade() -> Outcome<T>` — статика готова; РАНТАЙМ ГОТОВ 2026-09-26: `w.upgrade()` и `weak_upgrade(w)` оба возвращают Outcome<T> (ok несёт НАСТОЯЩИЙ acquire), `Outcome<T>`-мемберы `is_ok/is_err/value` типизированы на GenericInstance-приёмнике (check/call.rs, check/expr.rs, infer.rs), Weak-хендл доступен и через RawPtr-приёмник (weak_create типизирует RawPtr). Верификация: probe — upgrade ok → count 1→2, после release is_ok остаётся true (владение у Outcome)
- [x] Статический детектор циклов в ownership-графе: предупреждение с точным местом «поставь @weak здесь» — ГОТОВО: `detect_arc_cycles` (src/ownership/tmm.rs) в обоих пайплайнах (pipeline.rs:404/693), canonical-key dedupe, DFS-путь «A → B → A», @weak-ребро глушит цикл; `inspect --memory` подтверждён живым прогоном 2026-09-27
- [~] `@derive(AutoFree)` и `scope { }` блок: компилятор вставляет освобождения в конце scope; семантика defer-last — статический слой готов: has_autofree-детект атрибута, TmmLayer::AutoFree, collect_autofree_locals строит список локалов по функциям (tmm.rs ~470), `inspect --memory` печатает "Scope insertions (Layer 3, defer-last)" с release в обратном порядке. Кодоген-вставка освобождений — вместе с dataflow-слоем M6 (soundness требует точки последнего использования, не слепой конец scope)
- [x] FFI-маркеры: `import c "lib.h" with link(...)` + `owned`/`borrowed` на параметрах extern-функций; проверка на границе (borrowed не переживает вызов, owned передаёт владение) — ГОТОВО: `check_ffi_boundaries` → E-TMM-002
- [x] `forgen inspect --memory <file>`: аннотированный листинг — где что освобождается, какой слой выбран — ГОТОВО: TMM Memory Plan (типы+слои+поля, Scope insertions Layer 3 defer-last, рёбра+schedule)
- [ ] Замыкания: автоматическое определение захватов; захват, переживающий фрейм → объект-захват автоматически в @arc (Layer 3), остальное — Layer 1/2; пользователь ничего не пишет
- [ ] Тесты: циклический граф A→B→A с @weak (утечки нет), FFI owned/borrowed (нарушение = ошибка компиляции), AutoFree-порядок, @arc многопоточный (гонки — loom-style стресс на C-рантайме)
- [~] Замер накладных расходов: @arc vs raw — первичная таблица 2026-09-26 (Windows 11 x64, exe-build, n=30 process wall time, median): churn-петля 10k аллокаций @arc Box — 3.27 мс против raw Box 2.88 мс (+13%); полный метод изохронен. Полная таблица дереференсов — после M6 (нужны вставки release-on-scope для честного сравнения lifetimes)

## M6. TMM Layer 1: dataflow lifetime inference — ФЛАГМАН `[ ]`

- [ ] Анализ dataflow «последнее использование»: вставка free сразу после последнего использования, не в конце scope; use-after-free невозможен статически (точки освобождения вне достижимого использования)
- [ ] Автоперевод в arena для циклических структур (обнаружение цикла в графе владения → region/arena allocation автоматически)
- [ ] Инкрементальность: lifetime-граф кэшируется по модулям, перестройка только изменённых (иначе compile time убьёт фичу)
- [ ] Escape-политика: объект, ушедший за границу (возвращает функция/кладётся в долго живущий контейнер/захват замыканием) → план освобождения у владельца
- [ ] Подмножество гарантий v1.5.0: однонаправленное владение (деревья, DAG без циклов) — 100% покрыто; циклы → авто-arena или @arc/@weak-подсказка
- [ ] `forgen inspect --memory` показывает Layer 1-решения (прозрачность)
- [ ] Дифф-тесты: TMM-программы без аннотаций — компилируются, ведут себя идентично ручному управлению, 0 утечек (leak-счётчик runtime), 0 use-after-free (ASan-прогон тест-набора)
- [ ] Производительность: zero runtime overhead Layer 1 (подтверждение: нет счётчиков/барьеров в сгенерированном коде — инспекция Clif)
- [ ] Оценка объёма: 2–4 месяца полноценной работы — разбить на подмайстоуны внутри фазы (AST→dataflow-граф→решатель→эмит free→инкрементальный кэш→диагностика), каждый с зелёным гейтом

## M7. TSU: Type-Set Unboxing `[ ]`

- [ ] База: typechecker строит TypeSet { single, multi } flow-анализом присваиваний; сеты объединяются, ветвления сужают
- [ ] Оператор `is`: `if x is Int` — branch specialization; вырожденный сет в ветке → статический кодоген (одна проверка тега на входе ветки или ноль)
- [ ] E-ANY-диагностика: утечка val в статический контекст — ошибка с подсказкой
- [ ] Union-представление: { tag: u8, pad: 7, payload: 8 } = 16 Б, выравнивание 8; Int/Float/Bool — raw payload, Str/списки — указатель; НОЛЬ бокса примитивов
- [ ] Методы val: `.to_int()/.to_float()/.to_str()` (fail-loud), `.as_int()/.as_float() -> Outcome` (негромко)
- [ ] `List<any>`: гетерогенные списки, итерация + is-сужение
- [ ] Мосты: py.eval_any(...) -> val — динамический результат Python как val (главный потребитель TSU)
- [ ] Замеры: val-вырожденный vs let — Clif-дифф (ноль накладных); union-операции vs Python dynamic (цель ×3–10, честно); 16 Б против PyObject
- [ ] Тесты ≥ 10: вырожденный set байт-в-бит == let-версия; is-ветки; E-ANY; регресс существующих val-программ

## M8. Performance Sprint `[~]`

- [x] Loop-speed gap vs C (было ×13.9 на 100M-петле по Cranelift-замеру) — УСТРАНЁН 2026-09-27 в `src/optimizer/loops/ovf_elide.rs` (OverflowElision, выполняется только в OptTier::Speed — gate src/optimizer/mod.rs:367):
  1) Фикс-поинт диапазонов: op `&`/`|` добавлены в analyzable-set и в derive-матч — `i & 255` теперь даёт диапазон [0..255] (Range::bitand/bitor уже были, но не применялись).
  2) Loop-carried accumulator proof: для proven-индукции берётся back-edge feed параметра, по цепочке `+`/`-` BinOp от feed к параметру (термы с диапазонами) строится hull `[entry_lo + trip·x_lo, entry_hi + trip·x_hi]` (i128, checked_mul); если строго внутри i64 — feed и ВСЕ промежуточные звенья цепочки помечаются proven_no_overflow. Ключевая гарантия звука: помечаются ТОЛЬКО звенья аккумуляторной цепочки (не произвольные `+` с param-операндами — иначе `x + i` с неизвестным x был бы дырой).
  3) Вход-диапазон аккумулятора из const_ranges на entry-edges; фикс двойного счёта to_mark. Эффект — см. POST-FIX бенчмарк выше (Datara 4.72 мс < C -O3 8.39 < Rust 12.11).
  4) Codegen: LLVM Int BinOp при proven_no_overflow эмитит raw add/sub/mul вместо call datara_rt_checked_* (src/codegen/llvm/emit_inst.rs ~300-370); Cranelift — sadd_overflow+trapnz → iadd (inst_binop.rs ~362). 0 checked_add calls в IR петли; IR содержит llvm.loop.vectorize.enable width 8.
  5) Гейт звука: cargo test --release 1378/0, включая tests/test_opt_tier_overflow_elision.rs 5/5 (runtime-bound unit-тест, trap на реальном overflow, идентичность тиров bounded-loop). Без --opt=speed проверки переполнения сохраняются целиком.
  6) Аудит звука diamond_hull 2026-09-29: падавший тест `diamond_hull_beyond_i64_keeps_checks` разобран — пометки proven_no_overflow ставились ТОЛЬКО на индукционные инкременты `i+1` (wrapping_+, диапазоны внутри i64 — звук), а аккумуляторные добавки `s + 5e15` корректно НЕ помечены (`[ovf-branch] arms unresolved`); ассерт требовал пустое множество — слишком строгий. Тест переписан: (а) каждый помеченный ValueId — индукционный инкремент (константный операнд 1, резолв через копи-цепочки UnOp copy по всей функции), (б) поведенческий гейт — программа с реальным переполнением i64 обязана трэпнуться (non-zero exit) на ОБОИХ тирах. 8/8 зелёные. Дополнительно: `list_elem_kind_from_ty` чинил не только sort — см. §5-пункт sort-инициализации.
- [ ] PGO production: `--pgo-train` сбор профиля (база в src/pgo.rs есть) → `--pgo-use`: LLVM — реальные PGO metadata; Cranelift — branch-probability hints + hot/cold split
- [ ] Замеры PGO: vec/matmul/echo с PGO vs без (цель +5–15% на ветвистом коде); детерминизм результата с PGO == без
- [x] LTO: thin-LTO в release-пути LLVM — ИССЛЕДОВАНО И ОТКЛОНЕНО 2026-09-27 (решение документировано): A/B на 100M-петле, метод `llvm-as` → `rust-lld -flavor link` (LTO-линк биткода + datara_runtime.lib). Результат: размер 97 КБ против 212 КБ (runtime-мёртвый код вырезается), НО скорость 23.7 мс против 5.54 мс (×4.3 проигрыш): lld-LTO пайплайн отказывается векторизовать петлю («value that could not be identified as reduction is used outside the loop»), несмотря на сохранённые `llvm.loop.vectorize.enable`/`width 8` в IR — метаданные честно переносятся в биткод, но кодоген lld не применяет их к этой форме петли. Текущий путь (opt.exe -O3 → llc → link) векторизует и даёт 4.3–5 мс. Против принципа «никаких жертв скоростью» LTO-по-умолчанию не включаем; пересмотреть после перехода на clang-LTO путь (find_clang существует → `compile_with_clang` уже пробует -flto с fallback). Примечание 2026-09-27 (позднее): короткие имена символов в LLVM-IR (`slice_alloc` и пр.) — это НЕ «латентная проблема резолвинга», а симптом реального бага линковки LLVM-пути, закрытого ниже (см. чекбокс LLVM systems-primitives). `declare @destroy(i64)` без реализации — закрыт там же (no-op как в Cranelift).
- [x] LLVM systems-primitives: ЗОЛОТНИК-БАГ НАЙДЕН И ЗАКРЫТ 2026-09-27. Загадка «.ll с короткими именами vs .obj с datara_sys_*» решена ручным прогоном opt→llc→link: ренейма НЕ существует — короткие имена проходят весь пайплайн без изменений, НО MSVC link даёт LNK2019 (`slice_alloc`, `SliceView_set_byte/get_byte/free`), т.е. ЛЮБОЙ slice/destroy/volatile/bswap-билд на `--llvm` всегда падал на линке и ТИХО падал в фолбэк Cranelift (печатался вводящий в заблуждение баннер «LLVM pipeline completed»; obj с datara_sys_* в кэше — затёртый артефакт старого кодогена; пустое «LLVM Link step failed: .» — MSVC link пишет ошибки в stdout, а код читал только stderr). Фикс (вариант A, mirrors Cranelift declare_systems.rs): 1) runtime_decls.rs — declares заменены на реальные `datara_sys_*`/`datara_hw_*` имена (SliceView_*/VolatilePtr_*/голые slice/volatile/atomic/bswap/hton/ntoh/typed_zero_init убраны); 2) emit_call.rs — SliceView-методы мапятся в datara_sys_slice_*, VolatilePtr-методы в datara_hw_volatile_* (free → no-op: MMIO-вью не владеет памятью), свободные функции slice_alloc/slice_from_ptr/slice_subslice/slice_free + bswap/hton/ntoh/volatile_read*/volatile_write*/atomic_fence*/typed_zero_init мапятся в datara_sys_*/datara_hw_*; 3) destroy/drop → no-op `add i64 0, 0` (зеркало inst_call.rs iconst 0), голый `declare @destroy` убран из IR; 4) linker.rs compile_with_llc — ошибка линка включает stdout+stderr (LNK2019 больше не «пустая»); 5) resolver — `drop` зарегистрирован как builtin (резолвился E-RESOLVE-001 при том, что lowering и оба бэкенда его поддерживают). Проверено живьём: slice roundtrip, destroy/drop, bswap/hton/fence/typed_zero_init/subslice — все на НАСТОЯЩЕМ LLVM-пути (186 КБ, без LLVM-Warning в логе; фолбэк-Cranelift давал 209.5 КБ). Регресс-тесты: tests/test_v150_llvm_systems.rs 3/3 (LLVM-pipeline компиляция + запуск). Гейт: cargo build 0 warnings, cargo test --release 1381/0
- [ ] Codegen-аудит: comdat/section placement, const-pool шэринг, регистровое давление (по Clif-дампу); дедупликация строковых литералов в data-секции
- [~] Размер бинарников: база 2026-09-27 — `--tiny` работает и без потери скорости: hello int → 54.5 КБ (меньше C printf-gcc 87 КБ); 100M-петля → 114 КБ tiny (5.08 мс) vs 212 КБ release (4.72 мс) vs C -O3 85 КБ; runtime tiny-путь: clang -Oz -ffunction-sections/-fdata-sections + /OPT:REF,/OPT:ICF + NODEFAULTLIB + FILEALIGN:512 (src/codegen/linker.rs ~905). Полное расследование размера LLVM-пути 2026-09-27 (метод: /MAP-файлы линкера, dumpbin /SYMBOLS /HEADERS объектов архива):
  1) Статический CRT (libcmt+libucrt) тянет printf-движок UCRT (output.obj + float-форматирование _fptostr/cvt/cfout + EH-машина) = ~84 КБ .text — datara_print.o ссылается на __stdio_common_vfprintf/vsprintf, /OPT:REF это не выбрасывает.
  2) TLS-шаблон рантайма ~53 КБ (out_buf 16КБ + sso_ring 12КБ + int_bufs 8КБ + own_slots 8КБ + float-буферы 8КБ) — попадает в .rdata каждого exe; сокращение возможно (кольцевые буферы), но пока отложено из-за истории map-бага.
  3) opt.exe -O3 pre-pass перед llc ОБЯЗАТЕЛЕН для скорости (без него петля 95.7 мс — ломается векторизация), но расширяет obj 2.8КБ→13КБ+ и финальный exe на ~55КБ; это осознанный tradeoff speed/size.
  4) ДОБАВЛЕНО: DATARA_DYNAMIC_CRT=1 (src/codegen/linker.rs, Msvc-flavor link_args) — линкует msvcrt/ucrt/vcruntime импорт-стабы вместо статического CRT: 212 КБ → 114 КБ при 5.17 мс (n=15; без оптимизационных потерь, api-ms-UCRT DLL есть на Win10+). По умолчанию выключен (self-contained деплой). Верификация: cargo test --release 1378/0; examples 29/29 с DATARA_DYNAMIC_CRT=1; выходные значения совпадают.
  Следующие срезы: TLS-сокращение (−40 КБ потенциал), tiny-hello ≤ 40 КБ под вопросом (остаток — выравнивание секций + PE-заголовок).
- [x] Скорость компилятора: топ-горячее место найдено и устранено (2026-09-27). Профиль стресс-200k (115.3 c total): оптимизатор = 107.7 c (94%), из них `intraproc` 104 c за 3 итерации (26→34→43 c, растут). Причина: 10 guard-сканов `records.iter().any(...)` (5× engine_v2, 4× polyhedral, 1× cache_tiling) на каждую функцию × пасс × итерацию, а `records` растёт линейно с числом функций (22,980) → квадратичное поведение. Фикс: O(1)-маркеры `fn_pass_marked(pass, fn)` / `mark_fn_pass(pass, fn)` в OptimizationDecisionTrace (HashSet, serde-skip), заменяющие все 10 сканов; маркеры проставляются рядом с каждым успешным record пасса. Результат (seed 42, clean cache, Ryzen 5 7600): 200k check 115.3 c → **13.9 c (×8.3)**; оптимизатор 107.7 c → **7.4 c** (timeline: lowering 4.3 c → optimizer 11.7 c → backend-verify 12.8 c → total 13.4 c). Цель < 60 c достигнута с запасом. Гейт: build 0 warnings, cargo test --release 1381/0.
- [ ] Финальная таблица vs rustc -O3 (vec/matmul/reduce/строки/мосты) — свежий прогон, каждая строка с командой воспроизведения

## M9. Security + Fuzz + Supply chain (RC-гейт) `[ ]`

- [ ] cargo-fuzz таргеты: parser (произвольные байты), cimport lexer, bridge-декларации, datara.toml; CI fuzz-джоба (10 мин/таргет); найденные паники → fail-loud диагностика; 0 паник = гейт
- [ ] cargo-audit/deny: 0 известных CVE (или задокументированные false positives)
- [ ] Capability-аудит: тест-попытки обхода (path traversal `../` в fs-glob, symlink escape, env через bridge) — все блокируются, таблица попытка→результат
- [ ] unsafe-аудит: каждый unsafe в src/ имеет SAFETY-комментарий; сводка в SECURITY.md
- [ ] Supply chain: dpm lock с sha256 пакетов + транзитивных; reproducible add (два чистых прогона → идентичный lock); `dpm verify` ловит tamper-пакет (изменён байт → детект)
- [ ] Windows+Ubuntu CI зелёные; смоук бинарников Win x64 / Linux x64 / macOS arm64

## M10. NatVar async-рантайм `[ ]`

Решение владельца: отложить нельзя — нужен ядру браузера (сокеты/HTTP/таймеры), но делаем ПОСЛЕ каркаса M1–M4. Уникальная архитектура, НЕ tokio-клон (без раскраски, без state-machine boxing). Место: stdlib.async + C-рантайм (фиберы/поллер), пригодно и вне ядра (серверы, CLI).

- [ ] Фиберы: стек 4 КБ старт, динамический рост до 64 КБ; Windows CreateFiber, Linux/macOS ucontext; замер переключения (~10–20 нс цель)
- [ ] Scheduler: work-stealing, воркеры = ядра, локальная+глобальная очереди; I/O-поллер: epoll/IOCP/kqueue в отдельном потоке; socket_* builtins интегрированы (EWOULDBLOCK → парковка фибера)
- [ ] API: spawn(f) -> FiberHandle, yield(), sleep_ms(ms), join(handle) -> Outcome<T>; каналы фибер↔фибер и фибер↔поток (поверх механизмов v1.4.4); rt_mutex_lock -> Outcome (err="timeout"), deadlock-детектор `--detect-deadlock` (OFF по умолчанию)
- [ ] Бенчмарки vs tokio-референс на том же железе: echo 10k соединений (throughput, p99), 100k спящих фиберов (память), CPU reduce; если tokio быстрее — задокументировать ЧТО и ПОЧЕМУ, оптимизация патчами 1.5.x
- [ ] Тесты tests/test_natvar.rs ≥ 15: echo 1000 сообщений, параллельные фиберы == сумма, порядок yield/sleep, каналы, mutex timeout, deadlock-детект, 100k фиберов, паника в фибере → join.err, leak-счётчик, JIT+AOT
- [ ] Универсальность: API не привязан к ядру (обычный пользователь получает лёгкий async без раскраски — конкурентное преимущество против Go/Rust)

## M11. Gamedev/Core Pack I: окна+input+audio+ECS `[ ]`

Обоснование: это же фундамент UI-слоя будущего браузерного ядра (см. VISION §6) — пишем как переиспользуемые библиотеки, не «для игр».

- [ ] DPM-пакеты c:sdl2, c:raylib: bridge-декларации (window/events/renderer/audio), нативные либы через link в манифесте
- [ ] Тесты headless (SDL_VIDEODRIVER=dummy): окно открывается, цикл 60 кадров, input читается
- [ ] `stdlib.game`: fixed-timestep loop (аккумулятор, update/render разделение); ECS-идиома на struct+behavior + SoA-трансформер (уже есть в optimizer/adaptive) для hot-систем
- [ ] Демо: 10k сущностей движение+столкновения, 60 FPS на среднем железе (замер SoA vs AoS приложить)
- [ ] Audio: SDL2-мост, WAV/OGG через file_read_bytes, микширование, базовое позиционирование

## M12. Web groundwork: http-server + SSR + DOM-мост `[ ]`

Обоснование: SSR-шаблоны и HTTP-сервер — пролог к ядру браузера; DOM-мост готовит модель «Datara рядом с существующим вебом» (см. VISION §5–6).

- [ ] `stdlib.http` server поверх socket: маршруты, middleware, keep-alive; benchmark 10k RPS vs Go net/http (таблица)
- [ ] `stdlib.http` client: GET/POST/JSON (capability [Net])
- [ ] comptime-шаблоны SSR: типобезопасная интерполяция, авто-экранирование (XSS-тест: попытка через переменную → экранировано), raw-интерполяция явно; замер 10k страниц/с vs Go html/template
- [ ] DOM-мост через js-бридж: querySelector/textContent/addEventListener/attributes — тонкие обёртки; модель: SSR на Datara + гидратация событиями
- [ ] e2e: SSR-вывод валиден (парсер-проверка), гидратация доставляет события (мок js-моста в тестах)
- [ ] `stdlib.json` full: парсер/сериализатор (путь: Datara-реализация замеряется vs C; если C быстрее — C под капотом, Datara-API)

## M13. Экосистема: sparks + dpm + bridges `[ ]`

- [ ] sparks publish e2e: собрать mathx/strx/jsonx из tests/fixtures как реальные пакеты → подписать Ed25519 → опубликовать в реестр (D:\DATARA\sparks) → `dpm search` видит → `dpm add` → `dpm install` → программа компилируется и запускает пакетный код
- [ ] Добить до ≥5 реальных пакетов (предложения: jsonx, strx, mathx, resultx (Outcome-хелперы), webx (HTML-эскейп/URL), timex)
- [ ] `dpm` polish: понятные ошибки (нет пакета/версии/подписи), lock-файл стабилен, `dpm init` интерактивный шаблон, документация dpm.toml схемы
- [ ] Bridges-матрица CI: c/python/js/rust/npm/zig/lua/csharp — smoke-тест каждого моста в CI (workflow_dispatch), таблица статусов в docs
- [ ] `forgen bridge check` — статическая проверка деклараций против заголовков/стабов (наследие плана B1)
- [ ] Python-мост оптимизация: hot-path без лишних копирований (проверить буферный обмен), замер до/после

## M14. Документация + сайт + релиз 1.5.0 `[ ]`

- [ ] README (EN) по структуре: установка со всех платформ → первая программа за 60 c → зачем язык (проблемы/решения) → уровни → тур по фичам → perf-таблица → contribution (как коммитить, как помочь)
- [ ] README_RU: полный зеркальный перевод
- [ ] docs/ в репо + GitHub Pages (docs/ папка, Pages из main): полный мануал — language tour (10 глав), полный синтаксис, stdlib reference, архитектура (как устроен компилятор и ПОЧЕМУ так), мосты, типы/функции/классы; ноль внешних JS, тёмная тема, лого
- [ ] Учебник examples/tutorial/*.dtr: каждая глава компилируется и запускается в CI
- [ ] `forgen doc` строит сайт из докстрингов без ошибок
- [ ] Migration guide docs/migration/1.4-to-1.5.md + `forgen migrate 1.4 1.5` (механические правки: let→val, String→Str и т.п., с отчётом)
- [ ] SECURITY.md: как репортить, SLA
- [ ] Release engineering: установщики (Windows installer обновить, install.ps1/sh проверить), CHANGELOG одной сводкой 1.4.5→1.5.0, smoketest установщика
- [ ] Объявление обязательной обратной совместимости с v1.5.0 (VISION §12, README-блок)
- [ ] Финальный прогон §2 (Definition of Done) — все пункты с доказательствами

---

## H. Горизонт после 1.5.0 (не забыть, не мешать)

Порядок открытый; каждый пункт — кандидат в отдельный план после релиза.

- [ ] ABI freeze + LTS: заморозить C-рантайм API, String ABI, layouts (List/Map/Outcome), bridge-примитивы, datara.toml-схему; docs/abi.md + abi-тесты; LTS-ветка 1.5.x (security+bugfix)
- [ ] Incremental everything: resolve+typecheck+watch (цикл «сохранил→увидел» < 200 мс), мутационные тесты инкрементальности
- [ ] Cross-platform matrix: Linux aarch64 (QEMU/нативный CI), macOS aarch64 полный, Windows aarch64 смоук, SysV ARM64 ABI-тесты; reproducible builds (два чистых билда → равные хэши); FreeBSD/OpenBSD проба
- [ ] HFT Pack: zero-jitter подтверждение (@pool без syscalls в hot path, джиттер < 5%), p50/p99/p99.9/99.99 методология, stdlib.market (fixed-decimal OHLCV, lock-free SPSC 10M сообщений), backtest-демо
- [ ] Debugger (DAP): DWARF-полнота (dwarfdump-аудит), datara_dap (breakpoints/step/stack/variables/conditional), VS Code launch.json; честное ограничение: --opt speed теряет шаги
- [ ] Reflection-lite + derive(Json/CBOR): comptime fields_of(T), @derive(Json) round-trip, CBOR; замер vs serde_json (цель паритет ±50%)
- [ ] Game Dev Pack II: wgpu-мост (wgpu-native через C-ABI), hot-reload игровой логики (хост+DLL, state snapshot), Arkanoid showcase
- [ ] БРАУЗЕРНОЕ ЯДРО (мегапроект, отдельный план): архитектура в VISION §6; язык должен быть готов (M1–M12 это обеспечивают); dtrml/dtrf-дизайн финализировать до старта
- [ ] Самохостинг stage2: перевод компонентов компилятора на Datara (стратегия Zig/Rust/Go; см. VISION §7) — начиная с утилит, затем фазы, не трогая стабильный путь

---

## 4. Реестр рисков

| Риск | Фаза | Смягчение |
|---|---|---|
| Layer 1 TMM сложнее прогноза (false positives → странные ошибки) | M6 | Подмножества гарантий, инкрементальный граф, inspect-прозрачность, разбивка на подмайстоуны |
| Compile time TMM | M6 | Кэш lifetime-графа по модулям с первого дня |
| NatVar отстаёт от tokio в замерах | M10 | Честная публикация цифр, оптимизация патчами; API-преимущество (без раскраски) само по себе ценность |
| Модульный runtime ломает ABI линковки | M0 | /OPT:REF уже режет; прогон DCE-теста на каждой итерации |
| Стресс 200k вскрывает системные коллизии символов | M4 | Interning+mangling заранее (в той же фазе) |
| sparks-реестр 0 пакетов = экосистема мертворожденная | M13 | Публикация seed-пакетов сразу после M1 (зависимость: пакеты на новом stdlib) |
| Регрессии от thin-LTO | M8 | Fallback-флаг, бисекция, замеры до/после на всех examples |
| Размазывание фокуса (геймдев/веб раньше ядра компилятора) | все | Порядок M0→M14 жёсткий; M11/M12 — библиотеки, не продукты |

## 5. Реестр багов (найдено по пути — сюда; чинится в своей фазе или немедленно, если тривиально)

- [x] `--tiny` бит-в-бит с default (починено 2026-09-21)
- [x] dead-code молчал в check (починено 2026-09-21)
- [~] Вывод типа результата метода stdlib-класса без аннотации: `val parts = StringUtils.split(...)` не даёт `parts` тип List<Str> в lowering (lookup_var_type пуст), элемент печатается как число. С аннотацией работает. Найдено 2026-09-25 (M1 text-срез); ЧАСТИЧНО ПОЧИНЕНО 2026-09-29 (list-протокол Call-иниты, см. M1-заметку выше и §5-пункт «sort-инит» ниже); остаток (произвольные stdlib-методы, infer для class-receiver method calls) — в M4/M7.
- [x] Sort-инициализация без аннотации теряла элементный тип (найден 2026-09-29, класс E0944 silent-wrong-data): `val s = l.sort()` на List<Str> → `s.get(0)` печатал адрес char*. Корень двухслойный: (1) `list_elem_kind_from_ty` (src/codegen/cranelift/backend/inst_method_call.rs:670) матчил только "Str", а lowering выдаёт repr "String" → сортировка шла Int-сравнением указателей; (2) `record_var_type` (src/types/check/stmt.rs:6) кладёт в fn_symbol_types СТЁРТЫЙ erasure `Class("List")`, lookup_var_type (src/dmir/lowering/mod.rs:576) берёт его раньше symbol_types, и list_elem_repr падал в "Int". Фикс: (1) `is_str = ty.contains("Str") || ty.contains("String")`; (2) `infer_expr_datara_type` (src/dmir/lowering/infer.rs) выводит `List(elem)` для list-протокол MemberAccess-callee (map/filter/sort/reverse/clear/insert_at/slice/collect, helper infer_list_elem_type), а Let/Mut/Const/Val/CompactBind-обработчик (src/dmir/lowering/stmt.rs) уточняет стёртый Class("List") до List(elem) — аннотация и более точный symbol_types по-прежнему выигрывают. LLVM-путь отдельного elem-kind для get не имеет (repr из того же lowering-ty), фикс действует на оба тира. Верификация: пробы str_sort_probe/sort_probe2/annot_chain/sort_print_probe на Cranelift+LLVM (apple/fig/pear, exit 0); cargo build 0 warnings; cargo test --release 1388/0 (новые 4 теста в tests/test_list_str_index.rs); clippy lib+all-targets 0 warnings; examples 29/29, showcase 8/8, stdlib_tests 21/21, смоук 09_error_propagation_question exit 0.
- [ ] РОСТ MAP ЛОМАЕТСЯ НА ≥513 ЗАПИСЯХ (пре-существующий, есть на HEAD 7fc446a): `while i < 513 { m.insert(fmt"key_{i}", i*3) }` → `m.len()` = 512 (заморожен), дальнейшие вставки теряются; при этом m.get("key_512") НАХОДИТ значение — противоречие между map[0] и содержимым. C-харнесс тех же вызовов (gcc, tests-вне-пайплайна) корректен: len=600 на 600 вставках. Точка ЛОМАЕТСЯ между cap 512→1024; трасса роста показывает old_cap=0/мусор в hdr->capacity при валидном magic — подозрение на pool-взаимодействие или слот-дисциплину map-переменных в бэкенде (AssignVar rebinding после роста). Найдено 2026-09-25 (M1/HashMap-стресс). Чинить в M4 — блокирует полный стресс HashMap.
  [ПОБЕЖДЁНО 2026-09-25] КОРЕНЬ: ключи map из fmt"..." жили в ПЕРЕРАБАТЫВАЕМОМ scratch-ring (~1MB ≈ 512 итераций в этом ворклоаде); после оборота вставляемый ключ «совпадал» по указателю-алиасингу с приписанными ключами старых записей → e-scan находил «дубликат» → overwrite без роста count. ФИКС: rt_map_own_key() — вставка копирует ключ в собственное владение (fast path, growth, no-header, null-path); дополнительно fresh-block рост вместо realloc-in-place (f6a437e). Верификация: 600 записей len=600, 2000 записей len=2000, get(key_1999)=5997, contains(key_777)=true, overwrite(key_500)=1 без роста len. Остаток: отдельный БАГ — Map<Int, ...> с Int-ключами принимается чекером, но rt_map_insert ждёт Str-ключ-указатель → segfault; нужен ключевой дискриминатор типов (M4/M7).
- [ ] `map.get(k) or X` / `?` на map.get — E0944 при codegen: чекер типизирует map.get как Outcome<T>, но рантайм datara_rt_map_get возвращает сырое значение (ABI-разнобой). Фикс по образцу list pop v1.4.1: datara_rt_map_get_outcome, возвращающий Outcome-объект. Найдено 2026-09-25 (M0/T1-матрица); чинить в M1 (stdlib.collections) или M4.
- [ ] `?`-распаковка Outcome<List<T>>: payload_repr='List' теряет тип элемента → `l[0]?` на List<Outcome<Int>> даёт E0944 (индексный элемент не получает класс в val_to_class). Найдено 2026-09-25; фикс — протаскивать полный GenericInstance в PropagationSite.payload_repr и var_to_class; фаза M4/M7 (infer).
- [ ] match по enum-параметру с `fmt"..."` в армах → E0944 `__tag` (класс enum теряется в val_to_class, когда тело армы содержит FormatStr; с константными Str-армами тот же match работает, без main — падает даже с Int-армами). Найдено 2026-09-25 при написании TUTORIAL Step 6b; фикс в M4/M7.
- [ ] `Map.new()` — статический конструктор дженерик-класса из stdlib-модуля резолвится в Unit/undefined; канон сейчас — литерал `{ }`. Найдено 2026-09-25; фикс в M1 (конструкторы коллекций).
- [x] E0944 на `q.pop() or -1` / `.first() or` / `.last() or` без импорта (найдено 2026-09-29, ПОЧИНЕНО 2026-09-30): or-lowering (expr_composite.rs:888) эмитит GetField is_success на Outcome-объекте, но класс Outcome объявлен только в stdlib.result. Фикс: авто-загрузка stdlib.result в src/driver/modules.rs по образцу checked_add/file_read_*. Подводный камень, стоивший итерации: `q.pop()` — это Expr::Call с MemberAccess-callee, а не голый MemberAccess; первый вариант детектора матчил только OrRecovery{expr: MemberAccess} и пропускал. Финальный детектор — expr_uses_checked_accessor: любой Call с pop/first/last (callee_hits), плюс рекурсия в callee/args/OrRecovery/ErrorPropagate/MemberAccess/Binary/Unary. Верификация: /tmp/probe3.dtr → `8 / -1 / -2 / 7 / 7`, exit 0, без единого `use`.
- [~] Вывод типа bare-generic коллбека (найдено 2026-09-30, M1-срезы; чинить в M4/M7): `fn iter_map<T,U>(items: List<T>, f: U) -> List<U>` при вызове `iter_map([1,2,3], x => x*2)` инферит U как ТИП ЛЯМБДЫ `(Int)->Int`, а не тип её результата → все call-сайты «expected List, got List». Работает: same-shape generic (g5 → List<Int>, fold → A), generic-сравнение `a <= b`, рекурсия, block-лямбды-один-блок. Обход: builtin List.map/filter (same-type), iter_fold (A/F-дженерики), именованные fn вместо лямбд. Заблокировано: свободные iter_map/iter_filter как generic-fn.
- [ ] BLOCK-ЛЯМБДА С if ВНУТРИ — тихий пустой вывод (найдено 2026-09-30): `val mx = (a,b) => { if b > a { return b } return a }; out mx(3,5)` → EXIT=0, stdout ПУСТ (компиляция проходит, вывод теряется). Та же лямбда без if — работает; тот же if в именованной fn — работает; после первой такой лямбды ВЕСЬ дальнейший вывод программы пропадает молча. Фаза: M4/M7 (infer/expr-compiler), приоритет высокий — silent failure.
- [ ] Строковая конкатенация ВНУТРИ лямбды — висячий указатель (найдено 2026-09-30): `["a","bb"].map(w => w + "!")` → мусор-адрес в строке (140700235464736!). `w` без конкатенации и `.to_upper()` корректны. Похоже на lifetime результата `+` в теле лямбды (scratch-ring). Фаза: M4/M7.
- [ ] builtin filter на List<Str> ПОРТИТ результат (найдено 2026-09-30): `words.filter(w => w.len() >= 2)` → get(0) ок, get(1) возвращает элемент-НЕ-прошедший-фильтр, get(2) → null-pointer panic (invariant violation в out). На List<Int> тот же filter корректен (4 из 4). Фаза: M4/M7 или immediate-фикс elem-repr в filter-lowering.
- [ ] Строковая конкатенация `acc + w` внутри generic-лямбды — висячий указатель (найдено 2026-09-30): iter_fold(["a","b","c"], "", (acc,w)=>acc+w) → «562800941858888» вместо «abc»; то же в builtin map `w + "!"` → адрес+"!". Конкатенация вне лямбд и .to_upper() внутри — корректны. Фаза: M4/M7.
- [ ] Манифест-шорткат `fs = ["read"]` никогда не парсился (найдено 2026-09-30): serde-алиасы CapabilitiesConfig только fs_read/fs-read (или fs_write/fs-write); старый ключ `fs = [...]` в [capabilities] молча игнорируется → E-CAP-001 при любом file_* в проекте с манифестом. Работает: явные fs-read = ["*"] / fs-write = ["*"] (stdlib_tests/datara.toml обновлён). Фикс: добавить serde-алиас/нормализацию «fs» → обе категории; фаза M1/M14 (мелкий).
- [ ] str_scalar_at принимает БАЙТОВОЕ смещение, а не кодпоинт-индекс (найдено 2026-09-30 при написании stdlib.text): на "χαίρετε" str_scalar_at(s,1) → «�» (обрезанный байт α), тогда как str_substring_cp(s,1,1) → «α» (datara_fs.c:1050 — memcpy от s+offset без cp→byte-трансляции). Обход сделан: text_char_at_cp/text_count_char через str_substring_cp. Фикс в M4/M7: либо документировать byte-offset контракт и переименовать, либо добавить datara_rt_str_scalar_at_cp с трансляцией.
- [ ] Русские комментарии в .dtr-файлах читаются кракозябрами при некоторых путях чтения (кодировка; низкий приоритет, фикс в M14 при ревизии examples)
- [ ] tmp_*-файлы в корне — чистить перед релизом (M14 чеклист)

## 6. Протокол выполнения

1. Одна фаза за раз; внутри фазы пункты сверху вниз.
2. Каждый `[x]` сопровождается: дата + команда + ключевой вывод (одна строка).
3. Никаких «должно работать» — только воспроизведённый вывод.
4. Замеры честные: если цифра хуже цели — записываем как есть и разбираем, почему.
5. Изменения публичных интерфейсов (AST/IR/CLI) в конце фазы сверять с migration-заметками (M14 соберёт).
6. Перед стартом фазы: `git status` чистый, гейт зелёный.