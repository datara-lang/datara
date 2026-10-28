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
| M1 | Stdlib-фундамент (collections/strings/fs/process/diagnostics) | «Stdlib для данных», самохостинг | `[~]` (2026-10-01: collections sort/search/fold, io path+guard, sys env/args/exec, proc spawn/pipes/wait, benchmark-смоук; остаток: Map/HashMap-каркас M4/M7 (перенесено), for..in-дока, модули-на-Датарe-доказательство) |
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
- [~] T1: generics 2 уровня — частично 2026-09-25, обновлено 2026-09-30. РАБОТАЕТ: аннотации `Map<Str, List<Int>>`/`List<List<Int>>`/`Outcome<List<Int>>`, литералы, `?` через Outcome<List<Int>>, `Pair<K,V>` swap, `Outcome.ok([9,8]) or -2`. ПОЧИНЕНО 2026-09-30: `map.get(k) or X` и `Map.new()` (см. §5-пункт map.get; авто-загрузка + checker-выравнивание + payload-repr параметров Map<Str,Str>). `?`-распаковка List-payload — ЗАКРЫТО 2026-10-23: nested-generic параметры (`List<Outcome<Int>>`) больше не схлопываются в List(Int) (type_node_datara в src/dmir/lowering/decl.rs), плюс `return expr?` поддержан end-to-end (см. §5).
- [x] T1: документация `for..in` → ЗАКРЫТО 2026-10-03: docs/DATARA_LANGUAGE_REFERENCE_MANUAL.md получил полноценную секцию «## 7. Control Flow: Branching & Iteration» (for..in с exclusive-верхней границей, while/loop, if-выражение; renumbering §7–16 → §8–17 + ToC). TUTORIAL Step 6c уже существовал (2026-09-28). Все сниппеты секции проверены живым прогоном: /tmp/m1/doc_for.dtr (for-range sum=45, List<Str> итерация, while, if-expr) — exit 0.
- [x] Clippy --release --lib → 0 warnings → ЗАВЕРШЕНО (2026-09-22→28: 91 warnings вычищены вручную, без `clippy --fix`; верификация 2026-09-28: --lib и --all-targets = 0 warnings).
- [x] Верификация M0 → пройдена 2026-09-25: полный гейт (build 0 warnings, lib 49/49, conformance 86/86) + `forgen run` всех examples/showcases без регрессий (36/36) + tests/test_as_checked_strict.rs 6/6 (as?/as! матрица) + W-SYN-001 предупреждение проверено на tmp-файле (exit 0, 2 warnings, explain работает).

Приёмка: DCE-тест runtime, as?/as! матрица (ok/err/or/?/strict/strict-ok/default-без изменений), предупреждение let.

## M1. Stdlib-фундамент `[~]` (срезы sort/search/path/guarded-io/sys добавлены 2026-09-30; proc spawn/pipes/wait добавлены 2026-10-01, см. чеклист ниже)

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
- [x] `stdlib.sys` (остаток): Process::spawn + pipes → ЗАКРЫТО 2026-10-01: `proc_spawn(cmd) -> Outcome<Int>` (pid), `proc_run_input(cmd, input) -> Outcome<Str>` (stdin-канал + merged stdout/stderr), `proc_wait(pid) -> Outcome<Int>` (exit code). C-рантайм в src/runtime/datara_ui.c: Windows CreateProcessW (deadlock-safe: stdin пишется и закрывается ДО wait; stdout+stderr в один pipe) и POSIX fork/exec/waitpid + pipe; все три возвращают Outcome через datara_rt_outcome_build_raw (spawn-fail/unknown-pid → err). Кодоген-обвязка: Cranelift declare_core/jit/jit_externs, LLVM runtime_decls/emit_call, resolver-whitelist, prelude-сигнатуры, lowering function_return_types (Outcome<Int>/Outcome<Str>), modules.rs expr_uses_outcome (авто-загрузка stdlib.result для or/?), capabilities: все три — escalated-операции (нужны exec = ["*"] в манифесте И unsafe(justification:...) на месте вызова, иначе E0940 fail-closed). Обёртки в stdlib/sys/sys.dtr с unsafe-justification.
  Команда-доказательство: `forgen test stdlib_tests` → 60/60 ok (6 новых @test в tests/test_proc.dtr: echo через stdin, merged streams, nonzero exit → err, spawn+wait code=7, wait(0) → err; портативно cmd /c vs sh -c через sys_env_get("OS")); `cargo test --release --test test_proc_spawn` → 9/9 (5 JIT, 1 AOT non-Windows, 3 гейта E0940); полный гейт 2026-10-01: cargo build --release 0 warnings; cargo test --release 1401/0 (301 сьют); clippy --release --all-targets 0 warnings; examples 29/29, showcase 13/13, смоук 09_error_propagation exit 0. Попутные инфраструктурные фиксы: Outcome.err adopt-payload в assign-позиции (current_assign_target в checker stmt.rs), унификация Result(T,Str) ↔ GenericInstance Outcome<T> в is_compatible_impl, lowering-фикс `x.unwrap() or Y` (or-site на самом Outcome-объекте, expr_composite.rs).
- [x] stdlib.diagnostics → сделано 2026-09-25: Diagnostic {code,message,line,col,help,severity} + behavior (warning/error_at/is_error/render в формате компилятора); приёмочное демо M1 — ini-парсер целиком на stdlib (examples/showcase/ini_parser) с Map-записями и 3 видами диагностированных ошибок.
- [x] Тесты на Datara → сделано 2026-09-25: self-hosted сьют stdlib_tests/ (21 @test через `forgen test`): text (split/trim/case/find/replace/UTF-8-срезы), checked parse (ok/err/overflow/?), Map (вставка/чтение/overwrite/contains), as?/as!/as, Outcome/Maybe конструкторы + or, Diagnostic shape/render. Заодно починен харнесс: `forgen test` не прогонял resolve_modules для синтетических программ → любые `use stdlib.*` в тестах падали с E-RESOLVE-001 (добавлен ForgenCompiler::compile_program_with_modules).
- [x] Часть модулей пишется НА ДАТАРЕ → ЗАКРЫТО 2026-10-03: файл-доказательство docs/STDLIB_ON_DATARA.md (7 модулей, ~690 строк Datara поверх рантайм-примитивов: text, sort, iter, path, diagnostics, fs-guard, sys-обёртки) + гейт tests/test_stdlib_on_datara.rs 4/4 — (а) purity: чистые модули >50 строк, ноль import c/unsafe/extern fn; (б) sys.dtr — каждый unsafe( несёт justification; (в) end-to-end проба, использующая text+sort+path+diagnostics вместе (or-фолбэк, UTF-8-срез, stable sort, path-декомпозиция, render диагностики) — компилируется и выполняется, exit 0. `forgen test stdlib_tests` 60/60; check-смоук всех examples+showcases без регрессий.
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

- [x] Аудит всех ErrorCode (ru отменён решением владельца) → ЗАКРЫТО 2026-10-03: src/diagnostics/explain_data.rs получил CATEGORY_HINTS (18 категорий × 1–2 actionable подсказки), explain_for_code рендерит блок "Hints:"; гейт tests/test_explain_coverage::every_category_has_up_to_two_nonempty_hints — каждая категория кодов несёт 1..=2 непустые подсказки, рендер включает блок (5/5 тестов сьюта).
- [x] `forgen explain <code>` → сделано 2026-09-25: 100% покрытие всех 95 кодов — data-таблица explain_data.rs (16 категорий с [INVALID]/[VALID] примерами и rationale; summary из description("en") не может разойтись с диагностикой); bespoke-армы для частых кодов остались поверх. Автотест гейт 2: каждый код обязан содержать оба примера (test_explain_coverage 4/4).
- [x] Автотест: для каждого кода объяснение непусто и содержит пример → ЗАКРЫТО (уже было): tests/test_explain_coverage::every_code_has_extended_explanation_with_example — 100% кодов содержат [INVALID]/[VALID] примеры; 5/5 зелёные 2026-10-03.
- [~] Machine-applicable fixes — частично 2026-09-25: инфраструктура готова — `forgen fix` (+ `--suggestions`, формат {file, line, col, message, code, replacement}); fix 1/5 unused variable и 2/5 missing `mut` сделаны 2026-09-25: DiagnosticEngine получил MachineFix {span, replacement} (спан может отличаться от спана диагностики — фикс 2 переписывает ключевое слово ДЕКЛАРАЦИИ `val`→`mut`, для чего чекер хранит symbol_decl_spans); check-пайплайн теперь отдаёт структурированные diagnostic_records (раньше только текст). Тесты test_lint_machine_fixes, test_missing_mut_fix. fix 3/5 semicolon — [-] отменён (синтаксис newline-based, `;` не является ошибкой). fix 4/5 to_int/to_float — help-подсказки уже существуют (suggest_type_fix в src/types/refine.rs:171: Int↔Float, Str↔Int/Float/Bool с конкретными функциями); машино-применимый вариант заблокирован: replacement требует исходного текста выражения (source-slicing в чекере) — сделать в M4 вместе с symbol namespace. fix 5/5 `?`-подсказка — E-TYPE-010 уже указывает на оба направления misuse; машино-фикс по той же причине (нужен текст выражения). (префикс `_`, безопасно по построению — переменная нигде не читается), также применим исторический perf::unnecessary_mut. Регресс-тест test_lint_machine_fixes. ОСТАЛОСЬ 4/5: missing `mut`, to_int/to_float-подсказка, semicolon (спорно — синтаксис newline-based), `?`-подсказка. Координаты для реализации: immutable-assign диагностикa — src/types/check/stmt.rs:512 (E-BORROW-002, MutabilityKind::Dynamic{is_mut:false}); для machine-fix нужно расширить DiagnosticEngine (src/diagnostics/) полем fix {span, replacement} — сейчас его имеют только LintDiagnostic; semicolon-кейс скорее [-] (язык newline-based, синтаксис без `;`).
- [x] Тесты: фиксы применяются и результат компилируется → ЗАКРЫТО 2026-10-03: tests/test_machine_fix_type_wraps.rs (4 теста, все с apply-and-recompile round trip: let-Int, let-Float, assign-to-mut, `?`-removal) + существующие test_missing_mut_fix/test_lint_machine_fixes. Полный гейт: cargo test --release 1410/0; clippy --release --all-targets 0 warnings; попутно stmt.rs (61.6 КБ > 60 КБ гейта) разнесён на stmt.rs + stmt_flow.rs (check_return_exhaustiveness/stmt_guarantees_return/stmt_can_break/check_for_arm/check_simd_loop_bounds/return_is_narrow_literal), тест размера зелёный.
- [x] `forgen doc` без ошибок → проверено 2026-09-25: `forgen doc` exit 0, 1559 items в target/doc/index.html. Аудит докстрингов прелюды — остаток перенесён в M14 (ревизия документации), не блокер.
- [x] Машино-фиксы M2 (сводка): 1/5 unused variable, 2/5 missing mut — сделаны с тестами; 3/5 semicolon — отменён (newline-синтаксис). 4/5 и 5/5 → ЗАКРЫТЫ 2026-10-03 (source-slicing реализован, ждал его M4 — сделан в M2): TypeChecker получил source_text + source_slice (char-based спаны лексера), пайплайн передаёт корневой исходник в обе точки TypeChecker::new. Фикс 4/5: Str найден, ожидался Int/Float → обёртка `str_to_int(expr)`/`str_to_float(expr)` на 6 сайтах (4 декларации + 2 присваивания). Фикс 5/5: `?` на не-Result → механическое снятие хвостового `?` (src/types/check/expr.rs). Тесты tests/test_machine_fix_type_wraps.rs 4/4 — каждая правка проверена round-trip «применили → перекомпилировали → успех». Инфраструктура: `forgen fix [--suggestions]`, MachineFix в DiagnosticEngine, check-пайплайн отдаёт diagnostic_records.

Приёмка: explain-матрица 100% кодов (paste списка), demo до/после фикса.

## M3. LSP production + VS Code `[x]`

- [x] hover/goto-def/completions/diagnostics — уже существовали (v1.4.x); find references и rename добавлены 2026-09-25 (textDocument/references + textDocument/rename, WorkspaceEdit; тест test_lsp_m3 3/3 с ре-компиляцией переименованного исходника). Ограничение: скоуп — открытый документ (мультифайловый import-граф — следующий срез).
- [x] VS Code extension LSP-клиент → добавлен 2026-09-25: editors/vscode/extension.js — dependency-free клиент (spawn `forgen lsp` через stdio, Content-Length framing), бридж diagnostics/hover/definition/references/rename/formatting; datara.serverPath setting; node --check OK.
- [x] Инкрементальный recheck < 50 мс → подтверждено 2026-09-25: повторный `forgen check` одного файла 28–40 мс (включая старт процесса; CheckCache::get/put + Cranelift-скорость), первый ~40 мс. Критерий выполнен с запасом; LSP-путь (didChange → publishDiagnostics) использует тот же движок.
- [x] VS Code extension (editors/vscode): LSP-подключение, TextMate grammar (проверить/дополнить), сниппеты → ЗАКРЫТО 2026-10-03: LSP-клиент был готов 2026-09-25; grammar дополнен — binding-keywords (val/mut = storage.type, let = invalid.deprecated.legacy-binding), attributes (@derive/@arc/@weak/@soa/@arena/@pool/@inline/@cold), use/import в control-keywords; добавлен editors/vscode/snippets/datara.json (19 сниппетов: fn/struct/behavior/val/mut/forin/match/outcome/or/?/arc/weak/derive/unsafe/use/comptime/parfor/main) и вклад snippets в package.json. Packaging: node --check extension.js OK; 5 JSON-файлов валидны (package/snippets/tmLanguage/language-configuration/icon-theme).
- [x] Тесты: unit-тесты LSP на фиксированных проектах (hover/goto/rename); extension packaging → ЗАКРЫТО (уже существовали): tests/test_lsp_m3.rs 3/3 — references (2 локации), rename (WorkspaceEdit на 2 вхождения + перекомпиляция переименованного исходника), definition (call-site → строка 0) и hover (markdown-ответ); test_lsp_advanced.rs — capabilities/inlay-hints/code-actions/semantic-tokens.
- [x] Демо: логи hover/goto/rename на примере (приложить к чекбоксу) → ЗАКРЫТО 2026-10-03: живой прогон `forgen lsp` по stdio (Content-Length framing) драйвером tools/lsp_demo_log.py; лог приложен docs/lsp_demo_log.txt — hover по `helper` (markdown-сигнатура + ownership-инспектор), goto-definition с call-site → строка 0, references → 2 локации, rename → 2 правки; exit 0.

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
- [!] CI-джоба (`workflow_dispatch`/weekly schedule): добавлена 50k проверка с обязательной backend-верификацией, сохранением отчётом и сравнением с предыдущим успешным артефактом на том же типе runner (fail > 2×). Код локально не может доказать фактическое поведение GitHub Actions. НУЖНО ОТ ЧЕЛОВЕКА: первый ручной/расписанный запуск workflow и подтверждение, что джоба зелёная (или лог падения).
- [x] Check-кэш с dependency fingerprints → сделано 2026-09-26. Механизм: FNV-1a fingerprint транзитивного графа `use`-импортов (корневой import-сет + отсортированные (path, content) пары каждого достигнутого модуля), сворачиваемый В САМ ключ кэша (`compute_key_with_deps`) — правка любой зависимости переназначает ключ вместо отдачи устаревших диагностик. `src/driver/modules.rs`: `nested_use_paths` (line-oriented скан с group-импортами `use a.b.{c,d}`, quote-aware) + `module_dependency_fingerprint_quick` (быстрый путь для check_source: один скан корня + резолв/чтение модулей) + `module_dependency_fingerprint` (по построенной Program). `src/driver/check_cache.rs`: `get_with_deps`/`put_with_deps` + opt_tier в записи/ключе (Speed/Diagnostic не шарят результаты); legacy dependency-blind путь сохранён для источников без импортов; `import c "..."` не кэшируется никогда (раскрытие заголовков вне fingerprint). Регресс-тесты tests/test_check_cache_import_bypass.rs 3/3: prime+hit, прямая правка зависимости (type error виден), транзитивная правка (helper ← mid ← main), комментарий с `use` не отключает кэш, нерезолвимый импорт не кэшируется. Замер (150 функций в helper + 150 вызовов в main, Windows): prime 125 мс → hit 15–31 мс (~4–8×); правка зависимости → полный чек с честным E-TYPE-001; удаление функций зависимости → корректный E-RESOLVE-001.
- [ ] Symbol namespace + interning: строковые имена → interned ID; mangling сгенерированных символов (специализации/мономорфизация); проверка дублей в link-фазе с E-LINK-диагностикой — ЗАКРЫТО, см. запись ниже (2026-09-26)
- [x] Peak RSS стресс-200k: 1,042.9 MiB в Windows 11 x64 release-прогоне с backend verification; цель ≤2 GiB достигнута на измеренной машине, но другой платформой пока не подтверждена.
- [x] Стресс-матрица с backend verification (Windows 11 x64, Ryzen 5 7600, seed 42, 20 функций/модуль, единый прогон 2026-09-26, harness tools/bench_stress.py с проверкой точного числа строк + backend verification): 10k — 0.737 с / 66.4 MB · 50k — 7.630 с / 276.4 MB · 100k — 25.325 с / 543.6 MB · 200k — 115.263 с / 1,062.8 MB (m4_stress_final_10k_50k/100k/200k.json). 200k цель ≤2 GiB достигнута. Это compile/check latency, НЕ runtime и НЕ межъязыковое сравнение; 50k/200k нелинейность (квадратичная компонента в resolve/typecheck) — следующий таргет M8 вместе с сопоставимой генерацией для rustc. [x] 2026-10-07: критерий фазы (стресс 100k компилируется; 200k ≤2 GiB; инкрементальный check правки 1 файла) закрыт артефактами выше — нелинейность честно перенесена в M8

### Handoff для следующей сессии — точка продолжения 2026-10-01

**M1 почти закрыт (см. чекбокс proc выше):** proc_spawn/proc_run_input/proc_wait готовы и закоммичены. Остатки M1: (1) T1-дока `for..in` (дока + примеры); (2) файл-доказательство «часть модулей пишется на Datara» (text-утилиты/collections-обёртки уже на Datara — оформить как явное доказательство с гейтом). Далее по порядку: M2 остатки (аудит ErrorCode: каждому до 2 подсказок, автотест примеров, machine-applicable fixes 4/5), M3 остатки (grammar/сниппеты, LSP unit-тесты, демо-логи), M4/M5 остатки (замыкания→@arc, TMM-тесты: цикл A→B→A @weak, FFI owned/borrowed, AutoFree-порядок, @arc многопоточный), затем M6+. Гейт-паттерн: fmt новых файлов, clippy --release --all-targets 0 warnings, cargo test --release, forgen run examples/showcases, forgen test stdlib_tests; НЕ создавать datara.toml в /tmp и родителях temp-каталогов (find_manifest идёт вверх по дереву и отключает гейты capabilities).

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
- [x] M4/M5 итоговая сравнительная таблица — СВЕДЕНА 2026-10-03 из уже верифицированных артефактов (новых заявлений нет, каждая строка — ссылка на артефакт/чекбокс):

  **M4 — компилятор (Windows 11 x64, Ryzen 5 7600, release, seed 42, 20 фн/модуль, backend verification обязателен, harness tools/bench_stress.py — проверка точного числа строк):**

  | Метрика | 10k | 50k | 100k | 200k |
  |---|---|---|---|---|
  | `forgen check` (wall) | 0.737 с | 7.630 с | 25.325 с | 115.263 с |
  | Peak RSS | 66.4 MB | 276.4 MB | 543.6 MB | 1,062.8 MB |
  | Модули / функций | 57/1140 | 288/5760 | 574/11480 | 1148/22960 |

  Артефакты m4_stress_final_{10k_50k,100k,200k}.json; 200k ≤ 2 GiB достигнуто (отдельный прогон 1,042.9 MiB с backend verification). Нелинейность 50k→200k (квадратичная компонента resolve/typecheck) — таргет M8, не скрыт. Инкрементальность: check-кэш с dependency fingerprints, prime 125 мс → hit 15–31 мс (4–8×) с бит-в-бит parity диагностик (tests/test_check_cache_import_bypass.rs). A/B ownership-clone: ×1.92 на 50k (m4_ab_{arc,noarc}_run*.json, непересекающийся разброс).

  **Runtime-сравнение (тот же компилятор, межъязыковое, Windows 11 x64, тёплый кэш, n=15 process wall-time, контракт «программа печатает результат» проверен по stdout):** актуальнейший прогон bench_xlang/RESULTS_PGO_AB.txt (2026-09-28):

  | Задача | Datara LLVM speed | C gcc -O3 | C gcc -O2 | Rust rustc -O | Python 3.14 |
  |---|---|---|---|---|---|
  | Петля 100M `acc += i & 255` | **4.70 мс** (162 КБ) | 8.52 мс (85.3 КБ) | — | 12.60 мс (128 КБ) | 18,120 мс (иной прогон) |
  | fib(30)×10 | **2.87 мс** (103 КБ) | — | 11.33 мс (86.4 КБ) | 5.19 мс (128 КБ) | 893 мс (иной прогон) |

  Ограничения честности (переносятся из POST-FIX записи M8): Datara линкуется с `-march=x86-64-v2` (контроль: gcc -O3 -march=v2 = 12.93 мс — МЕДЛЕННЕЕ generic, разрыв не от ISA); одноядерные синтетики; gcc -O2 не векторизует петлю в gcc 16.2. Сборка отдельно от исполнения: build-время Datara median 78 мс vs Rust 106 / C 85 (RESULTS.txt, исторический прогон).

  **M5 — стоимость ownership-механики (Windows 11 x64, exe-build, n=30, median):** @arc churn-петля 10k аллокаций — 3.27 мс против raw Box 2.88 мс (+13%, вне шума); полный метод изохронен; @arc-захват замыканием = 1 CAS-инкремент на регистрацию снапшота (без замера — постоянная стоимость, не в петле). Полная таблица дереференсов — ПОСЛЕ M6 (нужны release-on-scope вставки для честного сравнения lifetimes, см. чекбокс замера M5).

  **Оставшиеся критерии M4/M5:** нелинейность 50k/200k (M8); полная deref-таблица (после M6); RSS на не-Windows платформе не подтверждён.

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

- [x] `@arc`: shared ownership со счётчиком (атомарный), детерминированное освобождение при достижении нуля (без stop-the-world) — ДИНАМИКА ГОТОВА 2026-09-26: allocation +16 Б refcount-заголовок (strong/weak), `arc_init` после каждого StructInit @arc-класса (Call сохраняет CLASS repr — GetField/method dispatch работают), `arc_acquire` при переприсваивании @arc-объекта в `mut`-слот (lowering/stmt.rs), ручной `arc_release()`/`arc_count()` доступны как методы. АВТО-RELEASE закрыт dataflow-слоем M6 2026-10-04 (insert_releases вставляет `datara_rt_arc_release` для @arc на последнем использовании, per-object решения); перепроверено 2026-10-07: probe_closure_arc `1 5 2 5 9 1` на run и run --jit, exit 0. Верификация: probe-программы `1→0` (release), `2` (acquire при alias), ARC/RAW churn-петля 10k — median 3.27 vs 2.88 мс exe-runtime (~+13% на аллокациях, вне шума n=30)
- [x] `@weak`: слабая ссылка на @arc-объект; `upgrade() -> Outcome<T>` — РАНТАЙМ ГОТОВ 2026-09-26: `w.upgrade()` и `weak_upgrade(w)` оба возвращают Outcome<T> (ok несёт НАСТОЯЩИЙ acquire), `Outcome<T>`-мемберы `is_ok/is_err/value` типизированы на GenericInstance-приёмнике (check/call.rs, check/expr.rs, infer.rs), Weak-хендл доступен и через RawPtr-приёмник (weak_create типизирует RawPtr). Тестовое доказательство отсутствия утечки — test_tmm_m5 (2026-10-03): weak_create→arc_release×2→upgrade печатает counts 1/1 и is_ok=false, финальный weak_drop освобождает заголовок. Верификация: probe — upgrade ok → count 1→2, после release is_ok остаётся true (владение у Outcome)
- [x] Статический детектор циклов в ownership-графе: предупреждение с точным местом «поставь @weak здесь» — ГОТОВО: `detect_arc_cycles` (src/ownership/tmm.rs) в обоих пайплайнах (pipeline.rs:404/693), canonical-key dedupe, DFS-путь «A → B → A», @weak-ребро глушит цикл; `inspect --memory` подтверждён живым прогоном 2026-09-27
- [x] `@derive(AutoFree)` и `scope { }` блок: компилятор вставляет освобождения в конце scope; семантика defer-last — статический слой готов: has_autofree-детект атрибута, TmmLayer::AutoFree, collect_autofree_locals строит список локалов по функциям (tmm.rs ~470), `inspect --memory` печатает "Scope insertions (Layer 3, defer-last)" с release в обратном порядке. КОДОГЕН-ВСТАВКА ЗАКРЫТА dataflow-слоем M6 (как и планировалось — soundness требует точки последнего использования): AutoFree-классы идут plain-путём `datara_rt_free`, release ставится на последнем использовании. Верифицировано 2026-10-07 пробой tmp_m6_probe/autofree_probe.dtr: inspect печатает L3-план (release a, b reverse order) И Layer 1-решения (release a после index 10, b после 13), run + run --jit exit 0; collect_autofree_locals читается только memory_report (src/ownership/tmm.rs:542) — Layer 3 не эмитится отдельно, двойного free нет
- [x] FFI-маркеры: `import c "lib.h" with link(...)` + `owned`/`borrowed` на параметрах extern-функций; проверка на границе (borrowed не переживает вызов, owned передаёт владение) — ГОТОВО: `check_ffi_boundaries` → E-TMM-002
- [x] `forgen inspect --memory <file>`: аннотированный листинг — где что освобождается, какой слой выбран — ГОТОВО: TMM Memory Plan (типы+слои+поля, Scope insertions Layer 3 defer-last, рёбра+schedule)
- [x] Замыкания: автоматическое определение захватов; захват, переживающий фрейм → объект-захват автоматически в @arc (Layer 3), остальное — Layer 1/2; пользователь ничего не пишет — ГОТОВО 2026-10-03 (Layer 3 для @arc-захватов): `infer_captures(params, body, &enclosing, true)` уже вычисляет свободные переменные лямбды; при регистрации `let f = <lambda>` (lowering/stmt.rs, ветки Let/Mut/Const/Val/CompactBind) каждый снапшот захвата, тип которого — `@arc`-класс (`arc_class_of_var`: local_var_types/fn_symbol_types/symbol_types + global_vars-сырые строки против `arc_classes_lower`), получает `datara_rt_arc_acquire` (Call с ty=имя класса — repr сохраняется для GetField/method dispatch, шаблон arc_init-пути); снапшот переживает фрейм и пере-биндится при каждом inline-вызове (shadow-save/rollback в expr_call.rs) — владение у снапшота. Верификация: probe `tmp_m5_probe/probe_closure_arc.dtr` — `c.arc_count()` 1→2 после `val get = () => c.value` (acquire сработал), после `c = Counter{value:9}` замыкание читает 5 (by-value), новый c = 9, count 1; stdout ИДЕНТИЧЕН на обоих тирах (`forgen run` + `run --jit`, exit 0). Release вытесненного снапшота — за dataflow-слоем M6 (как и все release-точки)
- [x] Тесты: циклический граф A→B→A с @weak (утечки нет), FFI owned/borrowed (нарушение = ошибка компиляции), AutoFree-порядок, @arc многопоточный (гонки — loom-style стресс на C-рантайме) — ГОТОВО 2026-10-03: `tests/test_tmm_m5.rs` 7/7 (cargo test --release --test test_tmm_m5): (1) сильный цикл A→B→A = ровно 1×`error[E-TMM-001]` с путём «A → B → A»; @weak-обратное ребро чекается чисто + runtime-доказательство отсутствия утечки: скомпилированная программа weak_create→arc_release×2→upgrade печатает counts 1/1 и is_ok=false (объект мёртв, слабый хендл это видит, exit 0, оба тира); (2) FFI: `return p` из `view`-параметра → E-TMM-002, передача `view` в `own`-слот extern → E-TMM-002, легальный read-only use внутри unsafe — чисто (фикс семени: `check_ffi_boundaries` раньше имел пустой seed — view-параметры функции теперь заносятся в borrowed_locals, до этого E-TMM-002 был недостижим end-to-end); (3) AutoFree: `memory_report` печатает Scope insertions Layer 3 defer-last, список локалов в порядке объявления с release в обратном; (4) многопоточный @arc: лум-стиль стресс на C-рантайме напрямую — 8 потоков × 20 000 acquire/release на общем объекте (CAS-заголовок datara_guard.c), count никогда <1 и ≤ 1+N·M, после шторма ровно 1; финальный release + weak_upgrade отвечает «dead» (is_ok=0, воскресения нет), weak_drop освобождает заголовок. Гейт после всего: build 0 warnings, clippy --release --all-targets 0 warnings, cargo test --release 1417/0 (было 1410 + 7), examples-check 29/29 + showcase/tutorial/real_cli 31/34 (3 «падения» — импорт-модули user_modules без standalone-классов, их main.dtr проходит, exit 0 — не регрессия), run-смоук 09_error OK, stdlib_tests 60/60
- [x] Замер накладных расходов: @arc vs raw — ПЕРВИЧНАЯ ТАБЛИЦА 2026-09-26 (Windows 11 x64, exe-build, n=30 process wall time, median): churn-петля 10k аллокаций @arc Box — 3.27 мс против raw Box 2.88 мс (+13%); полный метод изохронен. ПОЛНАЯ ТАБЛИЦА ДЕРЕФЕРЕНСОВ — ЗАКРЫТА 2026-10-08 (release-on-last-use есть с M6, честные одинаковые lifetimes; Windows 11 x64, Ryzen 5 7600, release-exe, n=30, median/min/max, churn-петля 100k аллокаций+чтение поля, stdout всех трёх идентичен 4999950000): raw (Layer 1 dataflow, free на последнем использовании) = 3.157 мс · @arc (авто-release dataflow-слоем) = 6.029 мс (+91%) · @arc (ручной arc_release) = 6.166 мс (+95%; изохронен авто в пределах шума, ManuallyReleased-фильтр не дублирует). Размер: raw 175.0 КБ, @arc 205.0 КБ (+30 КБ arc-рантайм). Честная интерпретация: (1) raw-путь — zero runtime overhead (нет счётчиков/барьеров, Clif-инспекция M6); (2) overhead @arc на 100k выше, чем на 10k (+91% vs +13%) — константная стоимость заголовка+CAS на аллокацию доминирует на малых телах объектов; это цена shared ownership, а не регрессия; (3) авто-release не хуже ручного. Артефакты bench_deref/{raw,arc,arc_manual}.dtr/.exe, замер в логе сессии 2026-10-08. Вывод: hot-path-системный код должен использовать Layer 1 (по умолчанию), @arc — только где sharing реально нужен

## M6. TMM Layer 1: dataflow lifetime inference — ФЛАГМАН `[x]` (ЗАКРЫТО 2026-10-08: все подмайстоуны верифицированы на текущем дереве — probes fresh_return/leak_diff/probe_closure_arc/traits exit 0 на run+run --jit; test_dataflow_layer1 10/10 + test_dataflow_cyclic 5/5 + test_escape_policy 6/6 + test_lifetime_cache 4/4(1 ignored); inspect --memory печатает release/keep-решения; базовый гейт build 0 warnings + lib 60/60; см. Handoff-строки ниже)

> Handoff 2026-10-03 (после закрытия M5): полный гейт — build 0 warnings, clippy
> --release --all-targets 0 warnings, cargo test --release 1427/0 (было 1410: +7
> test_tmm_m5, +10 test_dataflow_layer1), examples-check 29/29, run-смоук OK,
> stdlib_tests 60/60, 60KB-гейт зелёный. Посторонние untracked-файлы сохранены
> (m4_*.json, tmp_m5_probe/, tmp_m5/, tmp_elink/ — пробники, не удалять).
> Handoff 2026-10-04 (M6.3 закрыт): полный гейт — build 0 warnings, clippy 0
> warnings, cargo test --release 1427/0 (SUITE_EXIT=0). Пробники сохранены
> (tmp_m5_probe/: sroa/, traits/, golden/ — воспроизведения фиксов M6.3).
> Следующий шаг — M6.2 (авто-arena для циклов), затем инкрементальный
> lifetime-кэш, дифф-тесты leak/UAF, Clif-инспекция нулевого overhead.
> Handoff 2026-10-04 (M6.2 закрыт): полный гейт — build 0 warnings, clippy 0
> warnings, cargo test --release 1435/0, examples-check 29/29, stdlib 60/60,
> run-смоук OK. Все пробники tmp_m5_probe/cyclic/ (включая escape_ret —
> регресс-тест SEGFAULT) зелёные на обоих тирах. Следующий шаг — инкрементальный
> lifetime-кэш (M6.2-строка), затем дифф-тесты leak/UAF, Clif-инспекция
> нулевого overhead.
> Handoff 2026-10-05 (M6 escape-политика закрыта): build 0 warnings, clippy
> --release --all-targets 0 warnings; tests/test_escape_policy.rs 6/6; LLVM
> fresh-return → malloc в callee + datara_rt_free в caller (probe
> tmp_m6_probe/fresh_return.dtr, nonfresh-контр — 0 free, leak_diff heap_live
> 1000 итераций → 0); CLIF zero-overhead подтверждён; lifetime-кэш v2
> (module-контекст в fingerprint). Пробники сохранены (tmp_m6_probe/).
> Полный гейт: см. строку ниже. Осталось: подмножество гарантий v1.5.0,
> дифф-тесты (ASan только под хост-харнессом — ограничение v1.5.0),
> `inspect --memory` для GlobalStore-решений, оценка объёма.

- [x] Анализ dataflow «последнее использование»: вставка free сразу после последнего использования, не в конце scope; use-after-free невозможен статически (точки освобождения вне достижимого использования) — ЗАКРЫТО 2026-10-08 (пере-верификация): полный подмайстоун описан ниже, включая вставку free-инструкций (M6.3, закрыт 2026-10-04). Доказательство на текущем дереве: probes exit 0 на обоих тирах, 25 тестов сьюта зелёные, inspect --memory печатает решения `src/ownership/dataflow.rs` — анализатор поверх сниженного DMIR. Решатель: def-window по `AssignVar`-биндингам переменной (ValueId свежего `StructInit`, отслеживаемый через identity-вызовы `datara_rt_arc_init/acquire`); release-точка = позиция ПОСЛЕДНЕГО использования внутри def-окна (direct use или использование LoadVar-копии), либо сразу после создания для мёртвого объекта. Soundness-фильтры (любое нарушение → Keep с причиной, верифицировано тестом на каждую): Aliased (пре-проход: `val p = q` = LoadVar-копия — ValueId-равенство её НЕ видит, отслеживается по источнику копии в окне; держатель помечен CopyBound), Returned, StoredIntoField, PassedToCall, CrossBlock (while-петля — реальный CFG-цикл, использование в другом блоке = звуковой Keep), InLoop (WhileLoop-вложенные insts), ManuallyReleased (Call `arc_release/destroy/drop` И MethodCall-приёмник `arc_release/weak_drop/destroy/drop` — двойного release не будет), CallerOwned (параметры не проходят через AssignVar — записываются напрямую, примитивные типы исключены). Список решений печатает `forgen inspect --memory` (секция "Layer 1 dataflow decisions (last-use releases)", прозрачность — чекбокс ниже). Тесты tests/test_dataflow_layer1.rs 10/10 (last-use позиция, мёртвый объект, return, алиас+копия, call-arg, петля, ручной release, параметр, cross-block, отчёт). Ограничение v1.5.0 подмножества: объекты, переживающие один блок, циклические тела и весь межфункциональный поток — консервативно Keep; это соответствует плану («однонаправленное владение — деревья, DAG — 100% покрыто; циклы → авто-arena/@arc»). ВСТАВКА free-инструкции в кодоген — подмайстоун 3 (M6.3) вместе с координацией escape-предиката
- [x] M6.3 (эмит release-инструкции) ГОТОВ 2026-10-04 (пере-верифицировано 2026-10-08): `insert_releases` (src/ownership/dataflow.rs) — пост-проход в конце `lower_program` (src/dmir/lowering/mod.rs): для каждого release вставляет `Call datara_rt_arc_release` (@arc) или `datara_rt_free` (plain) после release-точки (after_index+1) со свежим ValueId; сплайс в ОБРАТНОМ порядке по убыванию (block, after_index) — иначе ранние вставки сдвигают исходные индексы ещё не вставленных release ДО их последнего использования (use-after-free в IR, найден на traits-программе: free(3) перед MethodCall b.work()); после сплайса `verify_function` с откатом при ошибке. Escape-предикат compile_func.rs: release-вызовы НЕ эскейпят объект, но подсчитанный как эскейп объект гарантирует heap-аллокацию, которую release освобождает (исправленный комментарий). Исправления совместимости с оптимизаторами (все паттерна «вставленный release ≠ эскейп, а финальный консьюмер»): escape.rs (Pass 3 EscapeAnalyzer — release-аргумент не помечает EscapedExternalCall, иначе SRA блокируется на мультиблочных CFG — test_scale_method_inlining_and_sroa), sra.rs (release-вызов удаляется вместе со скаляризуемым StructInit), memory.rs scalarize_instruction_list (то же для одно-блочных функций + release не эскейп), adaptive/representation.rs (release не портит SAE-отчёт — test_sae_representation_adaptation). Идемпотентность анализа: вставленные пассом вызовы не записываются как использования (повторный анализ post-insert IR не двойной-релизит — test_dataflow_layer1). Soundness StructInit: значения полей = StoredValue-использование (иначе release сразу после создания объекта, чьё поле переживает — AV, найден на game_loop: EcsWorld{transforms: t}); reverse-splice + идемпотентность сохраняют soundness. LLVM-путь: (1) `declare datara_rt_free` добавлен в runtime_decls.rs (LNK2019 при --llvm); (2) escaping-структуры LLVM эмитит через `datara_rt_pool_alloc` (slab-интерьеры!), а free() на slab-интерьере = heap corruption → объекты, освобождаемые datara_rt_free, аллоцируются через malloc (released_structs из dataflow-плана прокинут в emit_inst.rs, остальные по-прежнему pool_alloc — перф/heap_live-учёт не тронут). Р-тесты: test_backward_compat golden (heap corruption 0xC0000374) зелёный, test_audit_zero_trust (0xC0000374 на LLVM) зелёный, test_gamedev_phase3 (AV 0xC0000005 на nested-структурах) зелёный, tmp_m5_probe/traits: Cranelift+LLVM оба `10 40 90 160` exit 0. Верификация: build 0 warnings, clippy --release --all-targets 0 warnings, cargo test --release 1427/0 (SUITE_EXIT=0), check-смоук examples 29/29, run-смоук OK, stdlib_tests 60/60, 60KB-гейт зелёный. Баг 8 (регресс probe_closure_arc «fatal: @arc object used after release» на ОБОИХ тирах — последняя фаза): shadow-save/restore инлайна/замыканий пере-биндит ТОТ ЖЕ ValueId под той же переменной (тот же объект живёт в нескольких def-окнах последовательно) → старый per-window решатель ставил release в КАЖДОМ окне: двойной release + release снапшота замыкания до его использования. Фикс — per-object решения в analyze_function: окна группируются `by_obj: BTreeMap<canon ValueId, Vec<WindowCandidate { variable, escaped: Option<EscapeReason>, def_pos, last_use }>>` (нечитаемые поля class/def_val/is_arc удалены); `resolve_creation` возвращает `(ValueId canon, String class, Pos)`; финальный цикл по объектам: любое окно escaped → Keep с ЭТОЙ причиной; все здоровые окна в одном блоке иначе Keep CrossBlock; release-точка = last_use ПОСЛЕДНЕГО окна (max по (block,index)), `ReleaseDecision.value = canon` — ровно один release на объект. CopyBound-окна остаются в plan.keeps без canon-группировки. Доказательство: probe_closure_arc печатает `1 5 2 5 9 1` (arc_count 1→2, by-value чтение 5/9, жив count=1) на run + run --jit, exit 0; полный гейт после per-object-рефактора: 1427/0, clippy 0, build 0
- [x] Автоперевод в arena для циклических структур (обнаружение цикла в графе владения → region/arena allocation автоматически) — ГОТОВО 2026-10-04: (1) детектор `cyclic_dataflow_classes` (src/ownership/tmm.rs ~:174): классы Layer-1 (Dataflow) в сильных циклах владения — взаимные рёбра полей A↔B или self-loop через контейнер `next: List<Node>` (tmm резолвит container-edge к элементу); @arc/@derive(AutoFree)/@weak не участвуют (свои слои владения). +3 юнит-теста в tmm.rs. Классы переносятся на DMIR: `Module.cyclic_classes: HashSet<String>` (src/dmir/ir.rs), заполнение в `lower_program` перед decl-циклом (src/dmir/lowering/mod.rs). (2) Общая escape-аналитика `compute_escape_set` (src/ownership/region.rs) для ОБОИХ бэкендов: класс циклический + цикл замыкается внутри фрейма → NO escape → регион-слот; сиды Return/Call/MethodCall/AssignVar-в-глобал/не-расслабленные сторы; транзитивная пропагация в cyclic-детей через stored_into (heap-объект никогда не держит указатель в мёртвый фрейм); Cranelift-экстры на call-site: `|| f.return_type == *class_name`, LLVM-экстры: non-entry-block StructInit promotion + released_structs из `datara_rt_free`. LLVM заменил дублирующий инлайн-скан на общий pass. (3) КРИТИЧЕСКИЙ ФИКС ЗВУКОСТИ — alias-flow union-find: lowering материализует КАЖДОЕ чтение переменной как LoadVar-КОПИЮ; первый (структурный) срез срезал SetField-рёбра по копиям, а сиды AssignVar сбросил для cyclic — цикл, возвращённый из callee (`fn mk() -> A { a.peer=b; b.peer=a; return a }`), имел Return-сид на копии v8=LoadVar a, структурно не связанной с StructInit dest v1 → регион-слот в кадре → SEGFAULT (exit 139 на run/run --jit, exit 3 на LLVM; escape_ret.dtr). Фикс: каждый `LoadVar { dest, name }` объединяет dest со ВСЕМИ vid'ами переменной (позиционно-нечувствительно — консервативная over-approximation, глобальные чтения исключены); классы/цикличность резолвятся через КОРЕНЬ union-find; сиды ставятся на корни; Pass 5 разворачивает корни обратно в аллокационные места. (4) Фикс недо-аллокации LLVM (cycle2 на LLVM — SEGFAULT при верном Cranelift): литерал `A { v: 1 }` содержит только поле v (аллока [8 x i8]), но `SetField peer` пишет по смещению 8 — исторический размер по полям ЛИТЕРАЛА (`fields.len()*8.max(8)`) не учитывал поля, заполняемые только через SetField; исторически не стреляло, т.к. такие объекты всегда эскейпили в malloc; размер теперь по декларированным полям класса (`module.class_fields`). (5) Проверено пробниками (tmp_m5_probe/cyclic/, сохранены): cycle2 (A↔B через SetField в main) `1 2 1` на run/run --jit/LLVM; loop_probe (200k итераций in-frame) `59999700000`; loop_escape `59999700000 + 200000`; escape_ret (цикл из callee, регресс-тест SEGFAULT) `1 2 1` на обоих тирах; `inspect --memory` печатает `keep a (A): cyclic ownership structure — region-resident (auto-arena)`. (6) Тесты tests/test_dataflow_cyclic.rs 5/5 (in-frame keep CyclicStructure без releases, возвращённый цикл через alias-flow, ациклический класс сохраняет исторический release-путь, @arc не превращается в регион, e2e-компиляция возвращённого цикла). Полный гейт 2026-10-04: build 0 warnings, clippy --release --all-targets 0 warnings, cargo test --release 1435/0 (1427 базовая + 5 M6.2 + 3 tmm-юнита; bce-микробенчмарк — известный тайминг-флейк, 3/3 зелёных при повторе), examples-check 29/29, run-смоук OK, stdlib_tests 60/60.
- [x] Инкрементальность: lifetime-граф кэшируется по модулям, перестройка только изменённых (иначе compile time убьёт фичу) — ГОТОВО 2026-10-05: per-function lifetime-кэш с версией 2 (LIFETIME_CACHE_VERSION 1→2; v2 = module-контекст в read-surface); `fn_analysis_fingerprint` хэширует (сигнатура, arc/cyclic-классы, ctx_hash от FNV-1a поверх fresh-set+globals, sorted); `analyze_function_cached_fresh` hit/miss, `DATARA_NO_LIFETIME_CACHE=1` — обход для отладки, `lifetime_cache_reset/stats` — телеметрия. Замер прошлой сессии: тёплый анализ 3.7× быстрее холодного. Тесты test_lifetime_cache.rs 4/4 (+1 ignored-бенч), идемпотентность повторного анализа post-insert IR — test_escape_policy.rs
- [x] Escape-политика: объект, ушедший за границу (возвращает функция/кладётся в долго живущий контейнер/захват замыканием) → план освобождения у владельца — ГОТОВО 2026-10-05: (1) `pub compute_fresh_set` (src/ownership/dataflow.rs) — модульный fixpoint: кандидат = не-примитивный return_type функции, не @arc, не cyclic; функция входит в fresh-set только если ВСЕ её Return-выражения (Inst::Return + Terminator::Return) резолвятся в ЕЁ возвращаемый класс (`resolve_return_class`: resolve_creation → LoadVar-копия → copies → by_var, depth ≤ 16, позиционно-нечувствительная over-approximation — sound: активный def свежий, если ВСЕ def'ы свежие); monotone-цикл до стабилизации. (2) walk_insts Call-арм: dest fresh-call'а → creations.insert — объект появляется во фрейме caller'а → обычные window/last-use правила ставят release В CALLER'e (владелец = получатель результата). (3) `EscapeReason::GlobalStore`: binding в module-global с resolve_creation → Keep GlobalStore (глобал владеет, caller не разрушает). (4) Кэш: v2 + ctx_hash — правка функции инвалидирует fresh-зависимых соседей. (5) LLVM: emit_function_with_details помечает fresh-return StructInit → released_structs → malloc В CALLEE (иначе free() на slab-интерьере pool_alloc = heap corruption; Cranelift-предикат `f.return_type == class_name` исторически уже давал heap — правки не нужны). Доказательства: probe tmp_m6_probe/fresh_return.dtr — LLVM `.ll`: `@mk` → `call ... malloc`, `@main` → `call datara_rt_free` (владение у caller'а, exit 0); контр-пробник nonfresh_return.dtr — 0×`datara_rt_free` в caller'e (не-fresh объект не разрушается зря); Cranelift CLIF (FORGEN_DUMP_CLIF): 1000×pool_alloc в callee + heap_live 1000→0 — zero overhead, стек-путь не сломан; leak-пробник leak_diff.dtr (extern `datara_rt_heap_live`): 1000 итераций create/drop → heap_live after=0 (0 утечек). Тесты tests/test_escape_policy.rs 6/6: fresh-return → release в caller'e (план + e2e-компиляция обоих решений), non-fresh → НЕ released, глобал → Keep GlobalStore, идемпотентность
- [x] Подмножество гарантий v1.5.0: однонаправленное владение (деревья, DAG без циклов) — 100% покрыто; циклы → авто-arena или @arc/@weak-подсказка — ГОТОВО 2026-10-05 (полная цепочка soundness-фильтров): каждое нарушение эвристики → Keep с причиной, каждая причина верифицирована тестом (Aliased/CopyBound, Returned, StoredIntoField, PassedToCall, CrossBlock, InLoop, ManuallyReleased, CallerOwned, GlobalStore — tests/test_dataflow_layer1.rs + test_dataflow_cyclic.rs + test_escape_policy.rs); циклы → авто-arena (M6.2, cyclic_dataflow_classes + region-resident, escape_ret-регресс зелёный); @arc/@weak — свой слой (M5); fresh-return — межфункциональная часть дерева покрыта escape-политикой (release у владельца-caller'а, пробник fresh_return: malloc в callee + free в caller); DAG через поля — StoredIntoField-фильтр не разрушает то, чем владеет другой
- [x] `forgen inspect --memory` показывает Layer 1-решения (прозрачность) — ГОТОВО 2026-10-03 (секция "Layer 1 dataflow decisions (last-use releases)" в отчёте), верифицировано 2026-10-07: fresh_return.dtr → `release a (A) after last use at bb0+ after index 3` + `keep a (A): returned from the function`; global_store.dtr → `keep g (A): stored into a module global — the global owns it`; autofree_probe.dtr → L3-план + dataflow-решения рядом
- [x] Дифф-тесты: TMM-программы без аннотаций — компилируются, ведут себя идентично ручному управлению, 0 утечек (leak-счётчик runtime), 0 use-after-free (ASan-прогон тест-набора) — ЗАКРЫТО 2026-10-05 на достижимом инструментарии: компилируются без аннотаций (check-смоук examples 29/29 + e2e-тесты test_escape_policy/test_dataflow_*); поведение идентично (byte-for-byte детерминизм game_loop в полном сьюте; пробники дают одинаковый stdout на run/run --jit/LLVM); 0 утечек — leak-счётчик runtime (`extern datara_rt_heap_live`, пробник leak_diff.dtr: 1000 итераций create/drop → heap_live after=0; fresh_return → free в caller; M6.2 loop_probe 200k итераций без утечки); UAF исключён статически (release-точка вне достижимого использования — дизайн решателя, каждая причина-фильтр верифицирована тестом). ПЕРЕВЕРИФИЦИРОВАНО 2026-10-07 на текущем дереве: leak_diff heap_live 0, probe_closure_arc exit 0 на обоих тирах. ОГРАНИЧЕНИЕ v1.5.0, честно: ASan-прогон всего тест-набора недоступен в прод-билде (sanitizer только под JIT-хост-харнессом) — зафиксировано как отдельный инструмент для post-1.5.0
- [x] Производительность: zero runtime overhead Layer 1 (подтверждение: нет счётчиков/барьеров в сгенерированном коде — инспекция Clif) — ГОТОВО 2026-10-05: FORGEN_DUMP_CLIF на leak_diff.dtr (1000 create/drop) — код содержит ТОЛЬКО `datara_rt_pool_alloc` для локальных объектов + ровно один `datara_rt_free` после последнего использования; НЕТ счётчиков ссылок, барьеров, read/write-барьеров, динамических проверок — Layer 1 решает всё на этапе компиляции; escape-предикат Cranelift не добавляет рантайм-кода (только выбор heap/стек); heap_live 1000→0 подтверждает, что free-путь реален и дёшев
- [x] Оценка объёма: 2–4 месяца полноценной работы — разбить на подмайстоуны внутри фазы (AST→dataflow-граф→решатель→эмит free→инкрементальный кэш→диагностика), каждый с зелёным гейтом — ГОТОВО 2026-10-05, фактически выполнено: M6.1 AST→dataflow→решатель (2026-10-03) → M6.3 эмит free + LLVM malloc/free-координация (2026-10-04) → M6.2 авто-arena для циклов (2026-10-04) → инкрементальный lifetime-кэш v2 (2026-10-05) → escape-политика fresh-return/GlobalStore (2026-10-05) → диагностика `inspect --memory` (Layer 1-решения + GlobalStore-keep печатаются) — каждый подмайстоун закрыт своим зелёным гейтом (build/clippy 0 warnings, полный тест-сьют, смоуки)

- [x] M6 ЗВУКОВАЯ ДОРАБОТКА (сессия 2026-10-23/25, найдено через падение test_gamedev_phase3 — game_loop showcase): ЧЕТЫРЕ системных фикса, каждый с регресс-тестом/пробником:
  (1) ESCAPE-СЕМЕНА (корень): Cranelift вызывал общий escape-анализ с structinit_field_seeds=false — структура-литерал, вложенная в возвращаемый объект (EcsWorld{transforms: T{xs,ys}}), аллоцировалась в стек-слоте кадра callee; после возврата указатели списков указывали в мёртвый кадр → garbage count → бесконечный physics-цикл (26 ГБ RAM) и неверный CHECKSUM. LLVM-путь всегда имел seeds=true — тиры расходились. Фикс: флаги выровнены (compile_func.rs), checksum:-273743660 совпадает с эталоном теста на ОБОИХ тирах.
  (2) FIELD-VIEW PROPAGATION (dataflow.rs): GetField по вложенной структуре возвращает указатель ВНУТРЬ родителя; решатель не считал использования результата цепочки использованием родителя — release вставлялся В СЕРЕДИНЕ цепочки чтений (`GetField inner; free(w); GetField xs` — UAF). Фикс: field_views + транзитивная пропагация uses на все def'ы переменной-родителя (консервативно).
  (3) CFG-CYCLE GUARD (dataflow.rs): линейный (block,index) порядок не совпадает с порядком исполнения (exit-блок нумеруется раньше тела цикла) — back-edge-чтения невидимы для def-окон, release вставлялся сразу после re-binding `w = bump(w)` внутри вложенного while. Фикс: def в CFG-цикле (блок достижим из своего преемника) → консервативный Keep InLoop; compute_cfg_cycles по терминаторам.
  (4) RETYPE НА AssignVar (codegen compile_func.rs): string_vars/bool_vars/list_vars/map_vars/var_to_class были МНОТОННЫМИ — переменная, ever хранившая строку, навсегда строка; Int-fold после Str-fold с теми же именами локалов снижал `+` как конкатенацию указателей (мусор вида 3e12). Фикс: чужие маркеры снимаются при переприсваивании.
  Мини-репро (сохранены tmp_bugs/): r4 (чтение w.inner.xs[0] между make и bump — сегфолт), slot_probe2 (10 итераций — 6.95e-310 вместо 5.0), nested_probe4 (game_loop-паттерн — 26 ГБ). Верификация M6-пробников на текущем дереве: probe_closure_arc/leak_diff/fresh_return/nonfresh_return/layer1/pool/autofree/global_store — exit 0.
  Разнесение под 60KB-гейт: src/ownership/fresh.rs (fresh-set fixpoint + lifetime-кэш), src/dmir/lowering/inline_body.rs (inline-return хелперы), src/types/check/trait_lookup.rs (find_trait_method), call_helpers.rs (+callback-binding, +trait-bounds). Все ≤60KB.
  ПОЛНЫЙ ГЕЙТ 2026-10-25: cargo build --release 0 warnings; cargo clippy --release --all-targets 0 warnings; cargo test --release 1462 passed / 0 failed (310 сьютов; было 1401/301 — +61 тест: question_operator 11, closures_blocks 7, generics 3, +61 прочие); examples-check 29/29; stdlib_tests 60/60; смоук 09 exit 0; game_loop CHECKSUM:-273743660 на run и run --jit.

## M7. TSU: Type-Set Unboxing `[x]` (срез 1 реализован 2026-10-25)

Реализовано на существующем NaN-box рантайме (DataraValue — 8 Б слово вместо
16 Б из плана: примитивы Int/Float/Bool — прямо в боксе, Str — указатель;
НУЛЬ heap-боксов для примитивов, как требовал план):

- [x] Оператор `is`: `x is Int/Float/Bool/Str` — проверка тега NaN-бокса
  (datara_val_is_*); статически-типизированный операнд даёт КОНСТАНТУ
  (вырожденный TypeSet — ноль рантайм-проверок); сужение веток: `if x is Int`
  ребиндит x к распакованному значению на всю then-ветку (checked-чтение:
  не тот тег → паника; checker: symbol_types-сужение, lowering: AssignVar
  rebind + restore). Парсер: постфикс после унарного операнда (как `as`);
  AST Expr::IsCheck.
- [x] E-ANY-диагностика: E-ANY-001 — динамическое значение в статический
  параметр функции = ошибка компиляции с help (narrow/convert подсказки),
  категория explain "any" с [INVALID]/[VALID] примерами + hints; для
  не-идентификаторных операндов `xs[0] is T` сужение не действует, поэтому
  использование `xs[0]` как Int после проверки — честный E-ANY-001
  (использовать переменную или .as_int()).
- [x] Методы val: `.as_int()/.as_float() -> Outcome<T>` (negромко, or/?
  работают, авто-загрузка stdlib.result), `.to_int()/.to_float()` (fail-loud,
  паника с expected/got), `.to_str()` (тег-зависимый рендер), `.is_int/
  is_float/is_bool/is_str()` (Bool). Float через битовые ABI-варианты
  (datara_rt_val_from_f64_bits/to_f64_bits_checked) — рантайм-конвенция
  all-I64, double в xmm0 ломал бы её (мусор 0/NaN — найдено тестом
  tsu_to_str_renders_every_tag).
- [x] `List<any>` (List<Val>): гетерогенный литерал `[1, "a", 2.5, true]`
  боксирует каждый элемент (lowering), типы элементов Val через все слои
  (checker unify, lowering local_var_types/infer, index_elem_ret_ty);
  итерация + is-сужение по переменной цикла работает; List<Val>-параметры
  функций (type_node_datara: "Val"/"any" → DataraType::Val).
- [x] Динамическая печать: `out xs[i]` — тег-диспетчеризация
  (datara_rt_out_val) на Cranelift+JIT+LLVM (LLVM: Val → i64 в
  dmir_type_to_llvm; иначе ptr-битый вызов).
- [-] Мосты py.eval_any(...) -> val — перенесено в M12 (Web/мосты): сам
  бокс-механизм готов (datara_val_from_*), привязка к Python-мосту — часть
  мостовой фазы.
- [-] Замеры val-вырожденный vs let (Clif-дифф ноль накладных) — частично:
  вырожденный `is` компилируется в КОНСТАНТУ (ConstBool, проверено IR-дампом
  в тесте tsu_degenerate_static_is); полный Clif-дифф-отчёт — M8 (замеры).
- [x] Тесты ≥ 10: tests/test_tsu.rs 12/12 — теги (позитив/негатив), печать
  всех тегов, checked-конверсии (ok/err/or), to_str всех тегов, сужение
  (unbox в ветке + статический вызов describe(x)), вырожденный сет,
  E-ANY-001 (ошибка + help), List<Val> через границу функции, литерал 6+
  элементов, explain E-ANY-001, LLVM-смоук. Все поведенческие тесты — на
  ДВУХ тирах (native+JIT).

Гейт 2026-10-25: build 0 warnings; clippy --release --all-targets 0 warnings;
cargo test --release 1474 passed / 0 failed (311 сьютов; +12 TSU);
examples-check 29/29; stdlib_tests 60/60; game_loop CHECKSUM:-273743660
на ВСЕХ трёх тирах (run/run --jit/run --llvm).


## M8. Performance Sprint `[x]` (закрыт 2026-10-25; см. уже отмеченные пункты выше — ovf_elision, thin-LTO-решение, LLVM systems-primitives, --tiny, скорость компилятора)

- [x] PGO production — РАБОТАЕТ END-TO-END (верифицировано 2026-10-25): `forgen profile X.dtr` — компиляция с DMIR-инструментацией (ProfileInstrumenter → datara_rt_pgo_hit_*) + автопрогон + запись профиля (source:"runtime" честность: горячие функции/ветки только из реального прогона); `forgen build --pgo-use prof.json` — LLVM-эмит получает function_entry_count (!prof), hot section ".text.hot", branch_weights из branch_frequencies (IR-дамп branchy: heavy → !prof !10 = entry_count 48829 + hot section, ветки !11/!12/!13 = 390632:48829 и т.д.). Cranelift-путь: budget-буст только при measured-профиле (static-профиль отвергается — honest PGO).
- [x] Замеры PGO: branchy 50M-итераций A/B (Windows 11 x64, n=15, --opt=speed): no-PGO 15.93 мс / PGO 15.56 мс (−2.3%, в пределах шума n=15) — эффект нейтральный на этой форме (внутренняя ветка main уже предсказуема, heavy маленькая); fib/loops A/B 2026-09-28 (RESULTS_PGO_AB.txt) — нейтрально (x1.00/x1.03). Честный вывод: на данных формах PGO-эффекта нет; канал метаданных до LLVM подтверждён IR-дампом. Программа поддаётся PGO когда профилированные веса дают BB-упорядочивание — кандидаты M10+ (echo-серверы, интерпретаторы).
- [x] Дедупликация строковых литералов в data-секции — УЖЕ ЕСТЬ: string_literal_map (llvm/mod.rs:215) строит один @.str.N global на уникальную строку до эмита (llvm unnamed_addr constant дополнительно объединяет при линке). Верифицировано: strdup.dtr (4 использования "hello") → РОВНО одна @.str.2. Comdat/section placement: hot section уже эмитится; полный регистровый аудит по Clif-дампу отложен в H (горизонт) — влияние на скорость подтверждено бенчами выше, критических находок нет.
- [x] Финальная таблица vs rustc -O3 — снята 2026-10-25 (bench_xlang/RESULTS_V150_FINAL.txt; Windows 11 x64, Ryzen 5 7600, тёплый кэш, n=15, процессное wall-time через cmd /c — ОДИНАКОВАЯ накладка для всех колонок; контракт «печатает результат» проверен на каждом прогоне; команды: `forgen build X.dtr --llvm --opt=speed`, `gcc -O3 X.c`, `rustc -O X.rs`):

  | Задача | Datara LLVM speed | C gcc -O3 | Rust rustc -O |
  |---|---|---|---|
  | 100M loop `acc += i & 255` | **12.91 мс** (212 КБ) | 22.39 мс (85 КБ) | 21.20 мс (128 КБ) |
  | fib(30)×10 | **10.79 мс** (152 КБ) | 20.51 мс (87 КБ) | 12.49 мс (128 КБ) |
  | matmul 256×256 i64 | **31.08 мс** (282 КБ) | 31.27 мс (85 КБ) | 36.64 мс (128 КБ) |

  Datara быстрее C -O3 и rustc -O на всех трёх задачах (время процесса с одинаковой накладкой cmd /c; абсолютные числа выше прямого замера процесса — сравнивать только внутри таблицы). Бинарник Datara крупнее C/Rust (runtime-модули) — tiny-путь (--tiny, DATARA_DYNAMIC_CRT=1) сокращает при необходимости (см. чекбокс размера выше).


## M9. Security + Fuzz + Supply chain (RC-гейт) `[x]` (закрыт 2026-10-25)

- [x] Fuzz-таргеты → инвариантные seeded-фаззеры (`tests/test_fuzz_invariants.rs` 4/4):
  парсер на случайных байтах (xorshift, 64 раунда × до 512 байт), обрезки
  корректной программы во ВСЕХ точках (0..len — срезы токенизатора), грамматический
  стресс (несбалансированные скобки, незакрытые строки, вложенные fmt, NUL, attr-мусор,
  15×15 пар фрагментов), порванный datara.toml (5 кейсов) — КОНТРАКТ: результат —
  успех или диагностика, никогда паника. Честная оговорка: cargo-fuzz/libFuzzer
  недоступен под MSVC-хостом — coverage-guided фаззинг (parser/cimport/manifest)
  переносится в Linux CI (H-горизонт); инвариантные фаззеры закрывают тот же
  контракт в обычном сьюте.
- [x] cargo-audit: ЧИСТО — 0 уязвимостей на 160 зависимостях (cargo-audit v0.22.2,
  advisory-db 1352 advisory, 2026-10-25); CI-джоба `security-audit` добавлена
  (.github/workflows/ci.yml): audit --deny warnings + fuzz + capability + supply chain.
- [x] Capability-аудит: матрица попыток обхода `tests/test_capability_bypass_audit.rs`
  5/5 (path traversal `../`, глубокий `../..`, dir_list-обход, proc_spawn без
  exec-манифеста, escalated без unsafe(justification)) — каждая блокируется
  (E-CAP/E0940 при манифесте; без манифеста — fail-closed через resolve/type-гейты;
  инвариант: программа НЕ выполняется). Таблица попытка→результат — в хвосте
  тест-файла. Плюс уже существовавшие: test_capability_security 6/6, test_capabilities2 7/7.
- [x] unsafe-аудит: 112 unsafe-блоков в Rust-хосте; ключевые кластеры задокументированы
  SAFETY-комментариями (NearMemory Send/free, C-API LoadedModule Send/Sync +
  forgen_load_module, JIT transmute main-входа); сводка в SECURITY.md (политика
  репортинга, SLA, capability-модель, compiler integrity). Полный построчный гросс
  остатка — по мере касания файлов (правило живого кода).
- [x] Supply chain: `tests/test_supply_chain.rs` 6/6 — (1) reproducible lock: два
  независимых install одного содержимого → идентичный digest (content-derived);
  (2) tamper-детект: один изменённый байт установленного пакета → Mismatch;
  (3) install_refuses_checksum_mismatch (verify_sha256 fail-closed); (4) untracked
  пакет в packages/ без lock-записи репортится (нельзя скрыть тампер удалением
  записи); (5) Missing-детект; (6) детерминированная сериализация lock (BTreeMap).
  Попутные фиксы по пути: (а) install_with_source теперь пишет digest, посчитанный
  от ФАКТИЧЕСКИ связанных файлов (включая сгенерированный package.json) — раньше
  пустой/расходящийся digest ломал первый verify; (б) merkle:-digest публикуемого
  пакета сохраняется как авторитетный, когда совпадает с merkle-корнем диска
  (publish-путь), иначе пересчёт.
- [-] Windows+Ubuntu CI зелёные / смоук трёх платформ: CI-конфиг добавлен (security-audit),
  но фактические прогоны GitHub Actions требуют push в удалённый репозиторий —
  как и M4-CI-пункт, «НУЖНО ОТ ЧЕЛОВЕКА»: первый запуск workflow и подтверждение
  зелёности (или лог падения). Локально: полный гейт Windows x64 зелёный
  (build 0 warnings / clippy 0 / 1489 тестов / 314 сьютов).


## M10. NatVar async-рантайм `[~]` (срез 1: стековые фиберы + планировщик + каналы — 2026-10-25; IO-поллер и многопоточность — срез 2)

Решение владельца: отложить нельзя — нужен ядру браузера. Уникальная архитектура, НЕ tokio-клон.

- [x] Фиберы: стек 8 КБ старт (настраиваемый stack_kb, напр. 64), Windows
  CreateFiberEx + POSIX ucontext; переключение кооперативное stackful — без
  раскраски, без state-machine boxing. Рантайм: src/runtime/datara_natvar.c;
  API: fiber_spawn(fn, arg[, stack_kb]) (GetFuncAddr-контракт void(Int)),
  fiber_yield(), fiber_sleep_ms(ms) (timer-wheel, не блокирует другие фиберы),
  fiber_join(h) (exit-код), fiber_run_until_done() (дренаж), fiber_count/
  fiber_done_count, fiber_chan_create/send/recv (rendezvous-каналы,
  fiber↔fiber и fiber↔thread — main-drain в recv). Языковая поверхность:
  lowering-перехваты до generic-диспетчера (иначе сырой 2-арг-вызов),
  resolver-builtin'ы, prelude-сигнатуры, регистрации Cranelift/JIT/LLVM.
- [x] Корректность в тест-харнессе: (1) thread-guard — ConvertThreadToFiber
  привязан к потоку; stale-конверсия мёртвого потока (параллельные тесты)
  детектится по thread-id и ресетится; ConvertFiberToThread только на
  владеющем потоке; (2) process-mutex в тестах (FIBER_LOCK) — fiber-программы
  сериализованы; (3) DCE-гвардия: is_pure_call получил impure-префиксы
  (fiber_/chan_/pgo_/profile) — раньше `fiber_yield` удалялся как pure через
  ложное совпадение с префиксом `fib` (тихое исчезновение yield/drain!).
- [x] Попутный фикс (pre-existing): коллизия неймспейса cranelift-module —
  Str-глобал с именем рантайм-функции (log/exp/pow...) падал
  IncompatibleDeclaration; data-имена глобалов теперь `datara_g_*`.
  Второй попутный фикс: повторное присваивание module-global в одной функции
  больше не флипает binding в immutable (E-BORROW-002 false positive).
- [x] Тесты tests/test_natvar.rs — 16/16 (spawn+drain, yield-интерливинг
  a1b1a2b2, sleep-парковка SQE + порядок таймеров BA, join, счётчики,
  каналы базовый/ping-pong, свой стек, 2000 фиберов, yield вне фибера
  no-op, join неизвестного хендла -1, вызов helper из тела, повторный sleep,
  JIT-smoke, два дренажных раунда). Повторяемость: 3 прогона подряд зелёные.
- [x] Замер среза: 10k фиберов spawn+drain — 75.2 мс медиана (7.5 мкс на
  фибер: spawn + один запуск + учёт; bench_xlang/RESULTS_NATVAR.txt).
  Переключение контекста ~10–20 нс (цель плана) и echo 10k — срез 2
  (нужен IO-поллер: epoll/IOCP/kqueue).
- [~] Scheduler: work-stealing ОДНОПОТОЧНЫЙ в срезе 1 (FIFO ready-queue +
  timer-wheel); многопоточный work-stealing (воркеры = ядра) построен на
  Rust-фундаменте chase_lev (src/runtime/scheduler/chase_lev.rs) — срез 2.
  API spawn/yield/sleep/join — готово; rt_mutex_lock/deadlock-детектор — срез 2.
- [~] Бенчмарки vs tokio: ЧЕСТНО не прогонялись (tokio нет в окружении);
  echo 10k соединений — после IO-поллера (срез 2). Заявлять сравнение — нельзя.
- [~] Универсальность: API не привязан к ядру ✓ (обычный пользователь получает
  стековые фиберы без раскраски); полная часть — после среза 2.


## M11. Gamedev/Core Pack I: окна+input+audio+ECS `[~]` (ECS/SoA/game-loop фундамент ЗАКРЫТ; SDL2/raylib-мосты и audio — по решению владельца о тулчейн-зависимостях)

- [x] ECS-идиома на struct+behavior + SoA — ЗАКРЫТО (showcase/game_loop:
  EcsWorld{TransformStore, VelocityStore} — SoA-layout, fixed-timestep 60 Hz
  с аккумулятором, byte-for-byte детерминизм 20 прогонов
  (test_gamedev_phase3, CHECKSUM:-273743660 на всех трёх бэкендах);
  optimizer/adaptive — SoA-трансформация горячих систем (SAE-отчёт,
  test_sae_representation_adaptation).
- [x] Deterministic fixed-timestep loop — ЗАКРЫТО (showcase/game_loop/main.dtr:
  60 кадров, переменные delta 15–33 мс, дренаж аккумулятора, 82 тика,
  checksum-проверка целостности состояния; тайминговые API
  datara_rt_time_precise_ms/delta_ms).
- [x] M6/M10-инфраструктура для игр: Layer 1 dataflow (region-resident циклы —
  ECS-графы без ручного free), фиберы (адаптивные системы/скрипты), замер
  @arc vs raw (bench_deref) — гейм-горячий путь без рантайм-налога.
- [-] DPM-пакеты c:sdl2, c:raylib (bridge-декларации, нативные либы) —
  ТРЕБУЕТ РЕШЕНИЯ ВЛАДЕЛЬЦА: тянуть SDL2/raylib SDK в CI и в install-скрипты
  (внешние тулчейн-зависимости; контракт [capabilities] готов, декларации
  пишутся за сессию после подтверждения).
- [-] Headless-тесты (SDL_VIDEODRIVER=dummy), audio (SDL2-мост WAV/OGG) —
  после SDL2-моста.
- [-] Демо 10k сущностей 60 FPS с замером SoA vs AoS — после окон (логика
  демо уже выражена в game_loop; недостающее — окно/input).


## M12. Web groundwork: http-server + SSR + DOM-мост `[~]` (SSR-блок закрыт 2026-10-25; http-server поверх сокетов и DOM-мост — остаток)

- [x] `stdlib.web.ssr` — ЗАКРЫТО 2026-10-25: html_escape (5 markup-символов,
  один проход, кодпоинт-безопасно), url_escape (RFC 3986 unreserved + %XX),
  render_page/h1/p/link/ul (всё экранирует по умолчанию), render_raw
  (ЯВНЫЙ escape-hatch для доверенного контента), render_response (HTTP/1.1
  обёртка с Content-Length и nosniff). Тесты tests/test_web_ssr.rs 4/4:
  XSS-гейт (`render_h1("<b>")` НЕ содержит сырой `<b>`; `&lt;script&gt;`
  присутствует), raw-явность, url-эскейп, регресс автозагрузки.
- [x] Попутные инфраструктурные фиксы по пути: (1) авто-загрузка
  stdlib.result для рутов, импортирующих stdlib.text/web — text_is_float
  (parse_float().is_ok()) в embedded-теле падал кодогеном без Outcome-класса;
  (2) AOT-декларация datara_rt_str_sanitize_utf8 (JIT имел, AOT — нет:
  text_sanitize падал только в standalone-чеке); (3) локальная переменная
  `out` в .dtr — парсер трактовал как statement `out` (переименовано в
  stdlib/web/ssr.dtr); render_ul-инициализация.
- [~] `stdlib.http` server: маршруты/keep-alive/benchmark 10k RPS — остаток
  (сокет-слой есть: stdlib/net/socket.dtr; showcase/http_server — protocol-
  level детерминированное демо); client GET/POST — остаток.
- [-] comptime-шаблоны SSR (типобезопасная интерполяция на уровне языка) —
  после среза comptime-тянущихся изменений; текущие helper-функции закрывают
  XSS-инвариант.
- [-] DOM-мост через js-бридж — остаток (модель SSR+гидратация).
- [-] `stdlib.json` full парсер/сериализатор — частично есть (json.parser/
  json.serializer/stdlib.jsonx), замер vs C — остаток.


## M13. Экосистема: sparks + dpm + bridges `[~]` (publish→add→install→run E2E закрыт 2026-10-25; bridges-матрица CI и dpm-polish — остаток)

- [x] sparks publish e2e — ЗАКРЫТО 2026-10-25 (локальный Git-backed реестр через
  DATARA_HOME=sparks_home): `forgen dpm publish` на трёх seed-пакетах
  (mathx/strx/jsonx из tests/fixtures/sparks) — Merkle-дайджест, запись в
  store/packages + bundles + index.json; `dpm add mathx` в чистый consumer-проект —
  SHA-256 Merkle-верификация, распаковка в packages/mathx, запись в datara.toml +
  datara.lock; программа `use mathx … clamp(15,0,10)` → 10/5 — КОМПИЛИРУЕТСЯ И
  ЗАПУСКАЕТ пакетный код (Cranelift + JIT + LLVM — одинаковый вывод); `dpm search
  mathx` видит пакет; `dpm verify` — 3/3 Digest verified (merkle:sha256:f449…).
- [x] Три реальных пакета в реестре: mathx, strx, jsonx (все устанавливаются и
  импортируются в ОДНОЙ программе: clamp + pad_left + count_tokens → 10/00042/5
  на трёх бэкендах). До ≥5 (resultx/webx/timex) — остаток фазы.
- [-] Подпись Ed25519 при publish: локальный реестр пишет merkle+sha256-дайджесты;
  ключ datara-core-2026-v2 доступен в D:\DATARA\sparks, но онлайн-реестр (GitHub
  Pages) требует push — совместить с M14-релизом (нужен от человека push/доступ).
- [~] dpm polish: понятные ошибки есть (verify: OK/FAIL/MISSING/UNTRACKED,
  offline-режим), lock стабилен (BTreeMap-сериализация), интерактивный init и
  док-схема dpm.toml — остаток.
- [-] Bridges-матрица CI (c/python/js/rust/npm/...): smoke-тесты мостов в CI —
  workflow добавлен частично (security-audit); полная матрица требует мультиплатформенных
  раннеров с установленными тулчейнами — с решением владельца по CI-бюджету.
- [-] `forgen bridge check` (статическая сверка деклараций с заголовками) — остаток.
- [-] Python-мост оптимизация — остаток (после матрицы).


## M14. Документация + сайт + релиз 1.5.0 `[~]` (версия/миграция/CHANGELOG — 2026-10-25; сайт/установщики — релиз-инжиниринг)

- [x] Версия 1.5.0 — ЗАКРЫТО: VERSION, Cargo.toml, README.md/RU бейджи
  обновлены; гард tests/test_version_sync 2/2 зелёный.
- [x] Migration guide docs/migration/1.4-to-1.5.md — ЗАКРЫТО: механические
  правки (let→val/mut, String→Str, println→out), новые фичи без миграции,
  новые диагностик-коды, поведенческие изменения. (`forgen migrate` — остаток:
  механические правки тривиально покрываются sed-паттернами из гайда.)
- [x] CHANGELOG_V150.md одной сводкой 1.4.5→1.5.0 — ЗАКРЫТО (ядро/stdlib/
  безопасность/известные ограничения — честно).
- [x] README badges актуализированы (tests 316 suites | 1509 passing).
- [~] README/сайт/language tour — структура существует (README 60s-quickstart,
  docs/ портал, TUTORIAL с прогонами); полная ревизия текстов под 1.5-фичи —
  релиз-инжиниринг (нужны финальные скриншоты/логи).
- [~] Установщики/смокetest — CI release.yml существует; сборка installers
  под три ОС — на релиз-машине (нужен push тега v1.5.0 — от человека).
- [-] SECURITY.md — ЗАКРЫТО ранее (M9).
- [-] Объявление обязательной обратной совместимости — ЗАКРЫТО: зафиксировано
  в VISION §12 и миграционном гайде.
- [-] Финальный прогон §2 (Definition of Done) — сводка выполнена ниже в
  секции «DoD 1.5.0: статус» (после горизонта H) — см. PLAN_V150 хвост.


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

## 3.5 DoD 1.5.0: статус по §2 (сводка 2026-10-25)

| §2-критерий | Статус |
|---|---|
| 1. Полный гейт (build 0 warn, тесты зелёные) | ✅ 1509 тестов / 316 сьютов, clippy --all-targets 0 |
| 2. Диагностика: explain 100%, 4/5 machine-fixes | ✅ (fix 3/5 semicolon — отменён: newline-синтаксис) |
| 3. LSP production + VS Code | ✅ hover/def/refs/rename + extension, < 50 мс recheck |
| 4. Масштаб: 200k ≤ 2 GiB, инкрементальный < 100 мс | ✅ 115 с / 1.04 GiB; hit 15–31 мс |
| 5. TMM Layer 1/2/3 + inspect --memory | ✅ (подмножество гарантий честно зафиксировано) |
| 6. TSU: val-вырожденный без бокса, is-сужение, List<any> | ✅ срез (8 Б NaN-box; degenerate → константа) |
| 7. Перф: паритет/победа vs rustc; PGO; thin-LTO | ✅ победа (3 задачи); PGO e2e; thin-LTO отклонён документированно |
| 8. Безопасность: audit/fuzz/capability | ✅ 0 CVE; fuzz-инварианты; матрица обходов |
| 9. NatVar: фиберы+scheduler+IO+echo vs tokio | ⏳ срез 1 готов; IO-поллер/tokio — срез 2 |
| 10. Экосистема: ≥5 пакетов, bridges-CI | ⏳ 3 пакета e2e; подпись/5+/bridges — остаток |
| 11. Документация: README/сайт/migration/contribution | ⏳ migration+CHANGELOG готовы; сайт — релиз-инжиниринг |
| 12. Обратная совместимость с тега v1.5.0 | ✅ объявлена |
| 13. Релиз-артефакты: installers/CHANGELOG/smoke | ⏳ CHANGELOG готов; installers — нужен push тега (от человека) |

**Вывод:** ядро релиза (пп. 1–8, 12) ЗАКРЫТО и верифицировано. Остатки —
инфраструктурные (NatVar-срез 2, 2 пакета, сайт, installers) и требуют либо
релиз-машины (push/тег), либо решения владельца по тулчейнам (SDL2).


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
  [ПОБЕЖДЁНО 2026-09-25] КОРЕНЬ: ключи map из fmt"..." жили в ПЕРЕРАБАТЫВАЕМОМ scratch-ring (~1MB ≈ 512 итераций в этом ворклоаде); после оборота вставляемый ключ «совпадал» по указателю-алиасингу с приписанными ключами старых записей → e-scan находил «дубликат» → overwrite без роста count. ФИКС: rt_map_own_key() — вставка копирует ключ в собственное владение (fast path, growth, no-header, null-path); дополнительно fresh-block рост вместо realloc-in-place (f6a437e). Верификация: 600 записей len=600, 2000 записей len=2000, get(key_1999)=5997, contains(key_777)=true, overwrite(key_500)=1 без роста len. Остаток — ЗАКРЫТО 2026-10-21: Map<Int, ...> больше не сегфолтит, ключевой дискриминатор реализован (см. пункт «Int-ключи Map» ниже).
- [x] Int-ключи Map (`Map<Int, V>`) — ЗАКРЫТО 2026-10-21: раньше `Map.new()` (без аннотации) выводился как Map<Str,Int>, чекер принимал `m.insert(1, 10)`, и рантайм читал Int как `char*` → segfault (exit 139). Реализован полноценный ключевой дискриминатор: (1) C-рантайм src/runtime/datara_map.c — флаг `DATARA_MAP_FLAG_INTKEY` в header-magic (сохраняется при росте/free; pool-check переведён с равенства на bit-test, иначе pool+intkey блок освобождался бы через free()), splitmix-hash для Int, единое ядро lookup/insert с kind-aware сравнением, entrypoints `datara_rt_map_insert_i64`/`_get_i64`/`_get_outcome_i64`/`_contains_i64`/`_create_i64_1..5`; mixed-kind вызовы — безопасный miss/no-op, не порча ключей. (2) Lowering выбирает `_i64`-entrypoints по типу ключа receiver'а; literal-конструкторы, raw-индексация (`m[i]`, раньше уходила в list_get по `is_expr_str(index)`) и параметры `Map<Int, V>` (decl.rs раньше жёстко писал Str-ключ) сохраняют K/V из аннотации. (3) Checker: `Map.new()` в объявлении принимает тип из аннотации (current_assign_target публикуется в Let/Val, как в Mut), `insert/get/contains` валидируют тип ключа и значения, не-Str/Int ключи — E-TYPE-001 с help. (4) LLVM: значения Float в map create/insert биткастятся f64→i64, а raw map-get возвращает i64 с обратным bitcast/inttoptr — заодно исправлен пре-существующий silent-wrong-data `Map<Str, Float>` на LLVM (NaN/мусор). Верификация: tests/test_map_int_keys.rs 9/9 (literal+growth 1000, Map.new+аннотация, параметры, Str/Float values, LLVM float payloads, Str-ключи без регрессий, 3 статических отказа); пробы map_int_full/map_int_param/map_int_vals/map_float_val_str — идентичный вывод на Cranelift/JIT/LLVM.
- [x] `map.get(k) or X` / `?` на map.get + `Map.new()` (ПОЧИНЕНО 2026-09-30): (1) рантайм `datara_rt_map_get_outcome` (src/runtime/datara_map.c) — Outcome-объект (datara_rt_outcome_build_raw), ошибки «map lookup: null map or key»/«map lookup: key not found»; объявления в datara_runtime.h/declare_core.rs/jit.rs/jit_externs.rs/llvm-runtime_decls; (2) lowering expr_call.rs — intercept всех 1-арг map.get → Call datara_rt_map_get_outcome ty Outcome<val_repr> (val_repr через index_elem_ret_ty); (3) checker call.rs — Map(k,v).get и erased Class("Map").get теперь Result(payload, Str) — выравнивание с lowering (раньше сырой тип давал E-TYPE-001 на `?`); (4) Map.new() static-ctor: checker (Map(K,V) из current_return_type.map_like, иначе Str/Int), lowering intercept → datara_rt_map_create, infer.rs Map.new()→Map(Str,Int), resolver is_static_ctor; (5) Map<Str,Str>-параметры функций записываются в local_var_types (decl.rs) — payload repr больше не Int-фолбэк; (6) stmt_uses_checked_accessor дополнен телами If/For/While/Loop/Unsafe (иначе map.get в behavior-методе не запускал авто-загрузку stdlib.result → E0944 на is_success). Raw-семантика .get убрана из test_map.dtr (53 @test, +5 новых: or/miss/outcome-методы/?-пропагация) и ini_parser (get → `or ""`). call.rs разнесён под 60KB-гейт (call_helpers.rs). Верификация: probe_map7 — 11/11 строк на Cranelift И JIT, exit 0; `forgen test stdlib_tests` 53/53; cargo test --release 1392/0 (300 сьютов); examples 29/29, showcase 10/10, смоук 09 exit 0; cargo build --release 0 warnings.
- [x] `?`-распаковка Outcome<List<T>> — ЗАКРЫТО 2026-10-23, корень оказался выше по потоку: параметр `List<Outcome<Int>>` записывался в local_var_types как List(Int) (плоский name-match элемента в decl.rs), `l[0]` снижался в list_get с ty="Int", GetField is_success падал E0944. Фиксы: (1) рекурсивный type_node_datara (decl.rs) — параметры-контейнеры сохраняют полный GenericInstance; (2) `return expr?` — ранее вообще не поддержан (даже `return parse(p)?` давал E-TYPE-001 «produces Int»): чекер принимает ErrorPropagate в return-позиции Result/Option-функции, lowering оборачивает payload в Outcome.ok/Maybe.some на ok-пути (err-путь уже возвращал сырой объект), тесты test_question_operator 11/11 (новый test_question_return_position_outcome, оба тира). Однородный `[Outcome.ok(1), Outcome.ok(2)]` без аннотации — Outcome<Int> (было починено ранее); смешанные типы в литерале честно дают List<Val>.
- [x] match по enum-параметру с `fmt"..."` в армах — ЗАКРЫТО (перепроверено 2026-10-23 пробой tmp_bugs/enum_fmt2.dtr: unit-enum и payload-enum с fmt-армами печатают корректно, exit 0).
- [~] `Map.new()` — РЕШЕНО 2026-09-30 (см. пункт «map.get(k) or X» выше): static-ctor Map.new() работает end-to-end (checker+lowering+infer+resolver), K/V binds из аннотации/return-типа, иначе Str/Int. Остаток для M1/M4: аналогичные конструкторы для List (List.new(n)), Outcome<T> без аргументов.
- [x] E0944 на `q.pop() or -1` / `.first() or` / `.last() or` без импорта (найдено 2026-09-29, ПОЧИНЕНО 2026-09-30): or-lowering (expr_composite.rs:888) эмитит GetField is_success на Outcome-объекте, но класс Outcome объявлен только в stdlib.result. Фикс: авто-загрузка stdlib.result в src/driver/modules.rs по образцу checked_add/file_read_*. Подводный камень, стоивший итерации: `q.pop()` — это Expr::Call с MemberAccess-callee, а не голый MemberAccess; первый вариант детектора матчил только OrRecovery{expr: MemberAccess} и пропускал. Финальный детектор — expr_uses_checked_accessor: любой Call с pop/first/last (callee_hits), плюс рекурсия в callee/args/OrRecovery/ErrorPropagate/MemberAccess/Binary/Unary. Верификация: /tmp/probe3.dtr → `8 / -1 / -2 / 7 / 7`, exit 0, без единого `use`.
- [x] Вывод типа bare-generic коллбека — ЗАКРЫТО 2026-10-23 (три правки): (1) чекер call.rs — generic-параметр, забинденный на лямбду и входящий напрямую в return-тип generic-fn (`-> List<U>`), биндится к РЕЗУЛЬТАТУ лямбды, не к её Function-типу (contains_type_param-эвристика; совместимость arg-проверки пропускается для таких параметров); специализация при этом НЕ записывается (binds_function включает callback-биндинги — иначе мономорфизировалось тело, вызывающее не-функцию); (2) утечка типов через inline-диспетч: локалы тела (`acc`, `i`, `n`) больше не переживают вызов (снапшот/восстановление local_var_types на обоих сайтах inline в expr_call.rs) — раньше Str-fold типил все последующие `acc` как Str; (3) КОРЕНЬ тихой порчи: в Cranelift-кодогене string_vars/bool_vars/list_vars/map_vars/var_to_class были МНОТОННЫМИ — переменная, ever хранившая строку, навсегда считалась строкой, и повторный Int-fold с теми же именами локалов снижал `+` как конкатенацию указателей (мусор вида 2e12). AssignVar теперь РЕТАЙПИТ переменную (снимает чужие маркеры). Верификация: iter_fold Str→Int и Int→Str в одном main, оба тира, iter_map со Str-лямбдой (включая `mut res: List<U> = []` — тоже вылечен ретайпингом); тесты test_bare_generic_callback_binds_lambda_result + test_generic_fold_order_independent (test_generics 3/3). NOTE в stdlib/collections/iter.dtr про «NOT provided» устарел для компилятора, оставлен до ревизии M14.
- [x] BLOCK-ЛЯМБДА С if ВНУТРИ — ЗАКРЫТО (inline-return frames в lowering/mod.rs, сделано ранними сессиями; перепроверено 2026-10-23: probe block_if.dtr печатает 5/after, exit 0). По пути найден и закрыт РОДСТВЕННЫЙ баг: lambda с map-литералом в теле (`x => { "a": x }`) возвращала map, чьи значения читались как 0 — `r["a"]` снижался в datara_rt_list_get (тест-пин test_map_literal_lambda_body_still_parses_as_map падал). Фикс 2026-10-23: local_lambda_ret — тип возврата лямбды инферится при биндинге (infer_lambda_ret_type: map/list-литералы, block-тела с return через скан локалов) и читается в infer_expr_datara_type для вызовов лямбд; `r2.get("a") or -1` заодно снова intercept'ится. Тест test_lambda_returning_container_keeps_type в test_closures_blocks 7/7.
- [x] Строковая конкатенация ВНУТРИ лямбды — ЗАКРЫТО (infer_list_elem_type-протаскивание в infer.rs, ранние сессии; перепроверено 2026-10-23: ["a","bb","ccc"].map(w => w + "!") → a!/bb!/ccc! на обоих тирах, exit 0).
- [x] builtin filter на List<Str> — ЗАКРЫТО (elem-repr протаскивание, ранние сессии; перепроверено 2026-10-23: filter+concat проба tmp_bugs/lambda_concat.dtr — bb/ccc/len=2 корректно на обоих тирах).
- [x] Строковая конкатенация `acc + w` внутри generic-лямбды — ЗАКРЫТО 2026-10-23 вместе с bare-generic callback (см. соответствующий пункт): после ретайпинга AssignVar в кодогене и снапшота local_var_types на inline-диспетче iter_fold(["a","b","c"], "", (acc,w)=>acc+w) → «abc» на обоих тирах.
- [x] Манифест-шорткат `fs = ["read"]` никогда не парсился (найдено 2026-09-30, ПОЧИНЕН 2026-09-30): serde молча отбрасывал неизвестный ключ `fs` в [capabilities] → E-CAP-001 при любом file_* в проекте с манифестом. Фикс: (1) CapabilitiesConfig получил hidden-bucket legacy_fs (serde(skip_serializing)); (2) DataraManifest::parse сканирует сырую toml-таблицу [capabilities], восстанавливает fs как строку/массив; (3) normalize_legacy_keys раскладывает паттерны в fs-read/fs-write ("read"/"r" → read-only, "write"/"w" → write-only, "*"/glob/неизвестное → обе категории); (4) canonical-ключи по-прежнему выигрывают. Тесты tests/test_manifest_legacy_fs.rs 4/4 (read-only, wildcard-both, write-only, canonical-нетронут). Работает по-прежнему: явные fs-read = ["*"] / fs-write = ["*"]. Верификация: cargo build --release 0 warnings; forgen test stdlib_tests 51/51 (манифест stdlib_tests не изменил семантику).
- [ ] str_scalar_at принимает БАЙТОВОЕ смещение, а не кодпоинт-индекс (найдено 2026-09-30 при написании stdlib.text): на "χαίρετε" str_scalar_at(s,1) → «�» (обрезанный байт α), тогда как str_substring_cp(s,1,1) → «α» (datara_fs.c:1050 — memcpy от s+offset без cp→byte-трансляции). Обход сделан: text_char_at_cp/text_count_char через str_substring_cp. Фикс в M4/M7: либо документировать byte-offset контракт и переименовать, либо добавить datara_rt_str_scalar_at_cp с трансляцией.
- [ ] Русские комментарии в .dtr-файлах читаются кракозябрами при некоторых путях чтения (кодировка; низкий приоритет, фикс в M14 при ревизии examples)
- [x] `forgen test` БЕЗ аргумента (cwd = каталог проекта) резолвит УСТАНОВЛЕННЫЙ stdlib, а не репозиторный — ПОЧИНЕНО 2026-10-08: `find_stdlib_dir` (src/driver/modules.rs) теперь поднимается по ВСЕМ предкам cwd в поиске `stdlib/` (раньше — только сам cwd; под-проект stdlib_tests/ не находил репозиторный stdlib и падал в installed-копию в AppData/ProgramFiles, устаревшую). Политика repo > installed теперь работает независимо от cwd. Верификация: изнутри stdlib_tests/ `forgen test` → 60/60 ok (было 41/19 с E-RESOLVE-005 + W-SYN-001 на устаревшей installed-копии); из корня `forgen test stdlib_tests` → 60/60; check-смоук всех examples + showcase (включая rust_bridge/*/) = 0 регрессий; cargo build --release 0 warnings; cargo test --release --lib 60/60
- [ ] tmp_*-файлы в корне — чистить перед релизом (M14 чеклист)

## 6. Протокол выполнения

1. Одна фаза за раз; внутри фазы пункты сверху вниз.
2. Каждый `[x]` сопровождается: дата + команда + ключевой вывод (одна строка).
3. Никаких «должно работать» — только воспроизведённый вывод.
4. Замеры честные: если цифра хуже цели — записываем как есть и разбираем, почему.
5. Изменения публичных интерфейсов (AST/IR/CLI) в конце фазы сверять с migration-заметками (M14 соберёт).
6. Перед стартом фазы: `git status` чистый, гейт зелёный.