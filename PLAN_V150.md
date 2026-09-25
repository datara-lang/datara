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
| M1 | Stdlib-фундамент (collections/strings/fs/process/diagnostics) | «Stdlib для данных», самохостинг | `[ ]` |
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
- [ ] Clippy-чистка 91 warning: вручную по lint-кодам (io_other_error, useless_conversion, unnecessary_map_or, doc_overindented_list_items, borrowed_box, map_entry...). ЗАПРЕЩЕНО `clippy --fix` вслепую (ломает E0382: rust_bridge/mod.rs:128, cli.rs:113). Каждый файл — сборка+тест.
- [ ] Устаревший VERSION-механизм: уже 1.4.5 (сделано 2026-09-21); добавить тест-гард `version_sync`: VERSION == Cargo.toml version.
- [ ] `decide`/`match` документация: examples уже показывают decide; добавить 2 примера в docs/TUTORIAL.md (не блокер, но 15 минут).
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
- [ ] Clippy --release --lib → 0 warnings (2026-09-22: 91 warnings, из них ~46 machine-applicable; НЕ применять `clippy --fix` вслепую — он ломает владение E0382 в rust_bridge/mod.rs:128 и cli.rs:113; чинить вручную по одному lint-коду: io_other_error, useless_conversion, map_or, doc_overindented и т.д.)
- [x] Верификация M0 → пройдена 2026-09-25: полный гейт (build 0 warnings, lib 49/49, conformance 86/86) + `forgen run` всех examples/showcases без регрессий (36/36) + tests/test_as_checked_strict.rs 6/6 (as?/as! матрица) + W-SYN-001 предупреждение проверено на tmp-файле (exit 0, 2 warnings, explain работает).

Приёмка: DCE-тест runtime, as?/as! матрица (ok/err/or/?/strict/strict-ok/default-без изменений), предупреждение let.

## M1. Stdlib-фундамент `[~]`

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
> List<Str> = ...`) — реестр §5, фикс в M4/M7.

Цель: на Datara можно писать реальные программы (и тесты TMM/NatVar, и будущий самохостинг) без хождения в C.

- [ ] `stdlib.collections`: аудит текущих List/Map/Set/Deque/PriorityQueue; добить полный API: HashMap (открытая адресация, robin hood), HashSet, BTreeMap, Vec (удаление/вставка/срезы/итераторы), сортировки (stable+unstable), бинарный поиск
- [ ] `stdlib.text`: UTF-8-корректные split/trim/find/replace/slice (по кодпоинтам, не байтам), parse_int/parse_float с checked-семантикой (-> Outcome), format-библиотека на базе fmt-спецификаторов v1.4.5
- [ ] `stdlib.io`: File read/write/append (capability [FS]-guarded, через существующие file_read_checked-примитивы), Path-утилиты (join/parent/extension, кроссплатформенные разделители)
- [ ] `stdlib.sys`: Process::spawn + pipes (обёртка над sys::exec_utf8 v1.4.1 + stdin/stdout стримы), env, аргументы
- [x] stdlib.diagnostics → сделано 2026-09-25: Diagnostic {code,message,line,col,help,severity} + behavior (warning/error_at/is_error/render в формате компилятора); приёмочное демо M1 — ini-парсер целиком на stdlib (examples/showcase/ini_parser) с Map-записями и 3 видами диагностированных ошибок.
- [x] Тесты на Datara → сделано 2026-09-25: self-hosted сьют stdlib_tests/ (21 @test через `forgen test`): text (split/trim/case/find/replace/UTF-8-срезы), checked parse (ok/err/overflow/?), Map (вставка/чтение/overwrite/contains), as?/as!/as, Outcome/Maybe конструкторы + or, Diagnostic shape/render. Заодно починен харнесс: `forgen test` не прогонял resolve_modules для синтетических программ → любые `use stdlib.*` в тестах падали с E-RESOLVE-001 (добавлен ForgenCompiler::compile_program_with_modules).
- [ ] Часть модулей пишется НА ДАТАРЕ (файл-доказательство, что язык тянет реальный код; начать с text-утилит и collections-обёрток поверх C-рантайма)
- [ ] Верификация: benchmark-смоук (collections на 1M операций — таблица vs naive), полный гейт

Приёмка: `forgen test` suite для stdlib; демо-программа «парсер ini-файла» чисто на stdlib без FFI.

## M2. Diagnostics Gold `[~]`

> РЕШЕНИЕ ВЛАДЕЛЬЦА 2026-09-25: диагностика — ТОЛЬКО на английском;
> двойная локаль (ru) из исходного чеклиста отменена. description()
> игнорирует параметр locale, русские строки удалены из codes.rs.
> Гейт 1 сделан: exhaustive-список ErrorCode::all_codes() (генерируется
> из энума) + автотест tests/test_explain_coverage.rs (3/3): все ≥90 кодов
> имеют непустое описание, строки кодов уникальны и имеют префикс E/W,
> якорные коды объяснимы. Гейт 2 (расширенные объяснения с примерами для
> КАЖДОГО кода) и machine-applicable fixes — остаток фазы.

- [ ] Аудит всех ErrorCode: каждому — код, сообщение (en/ru), span с подсветкой, до 2 подсказок
- [ ] `forgen explain <code>`: расширенное объяснение + ПРИМЕР ПРАВИЛЬНОГО КОДА
- [ ] Автотест: для каждого кода объяснение непусто и содержит пример
- [~] Machine-applicable fixes — частично 2026-09-25: инфраструктура готова — `forgen fix` (+ `--suggestions`, формат {file, line, col, message, code, replacement}); fix 1/5 unused variable (префикс `_`, безопасно по построению — переменная нигде не читается), также применим исторический perf::unnecessary_mut. Регресс-тест test_lint_machine_fixes. ОСТАЛОСЬ 4/5: missing `mut`, to_int/to_float-подсказка, semicolon (спорно — синтаксис newline-based), `?`-подсказка. Координаты для реализации: immutable-assign диагностикa — src/types/check/stmt.rs:512 (E-BORROW-002, MutabilityKind::Dynamic{is_mut:false}); для machine-fix нужно расширить DiagnosticEngine (src/diagnostics/) полем fix {span, replacement} — сейчас его имеют только LintDiagnostic; semicolon-кейс скорее [-] (язык newline-based, синтаксис без `;`).
- [ ] Тесты: фиксы применяются и результат компилируется
- [ ] `forgen doc` без ошибок на полном прелюде + stdlib; докстринги у всех встроенных функций (аудит prelude)

Приёмка: explain-матрица 100% кодов (paste списка), demo до/после фикса.

## M3. LSP production + VS Code `[ ]`

- [ ] hover (типы + докстринги), goto definition (кросс-файлы через import-граф), find references, безопасный rename (все точки + компиляция после), completions (методы по типу приёмника, встроенные с сигнатурами), diagnostics в реальном времени (переиспользовать forgen check API)
- [ ] Инкрементальный recheck одного файла < 50 мс (опора на кэш v1.4.4 + M4)
- [ ] VS Code extension (editors/vscode): LSP-подключение, TextMate grammar (проверить/дополнить), сниппеты
- [ ] Тесты: unit-тесты LSP на фиксированных проектах (hover/goto/rename); extension packaging
- [ ] Демо: логи hover/goto/rename на примере (приложить к чекбоксу)

## M4. Large-Program Stability `[ ]`

- [ ] Генератор стресс-программ (tools/gen_stress.py или Rust-бин): N модулей × M функций, перекрёстные импорты, дубли имён, глубокие цепочки; наборы 10k/50k/100k/200k строк
- [ ] CI-джоба (workflow_dispatch): стресс 50k, регресс-гейт по времени (fail > 2× baseline)
- [ ] Инкрементальный typecheck: resolve/typecheck кэшируются по файлу + хэшам зависимостей; правка 1 файла из 200 → check < 100 мс; корректность = бит-в-бит сравнение диагностик полной vs инкрементальной прогоны (серия мутаций)
- [ ] Symbol namespace + interning: строковые имена → interned ID; mangling сгенерированных символов (специализации/мономорфизация); проверка дублей в link-фазе с E-LINK-диагностикой
- [ ] Память компилятора: замер peak RSS на 200k; оптимизация топ-потребителей (клонирование AST, строки резолвера); цель ≤ 2 ГБ
- [ ] Таблица замеров 10k/50k/100k/200k (время фаз, RSS) + сравнение с rustc на эквивалентной генерации — приложить к чекбоксу

## M5. TMM Layer 2+3: аннотационный каркас `[ ]`

Полный дизайн — VISION §3. Слой 2/3 делаем ПЕРВЫМ, чтобы Layer 1 (M6) опирался на готовую машинерию.

- [ ] `@arc`: shared ownership со счётчиком (атомарный), детерминированное освобождение при достижении нуля (без stop-the-world)
- [ ] `@weak`: слабая ссылка на @arc-объект; `upgrade() -> Outcome<T>`
- [ ] Статический детектор циклов в ownership-графе: предупреждение с точным местом «поставь @weak здесь»
- [ ] `@derive(AutoFree)` и `scope { }` блок: компилятор вставляет освобождения в конце scope; семантика defer-last
- [ ] FFI-маркеры: `import c "lib.h" with link(...)` + `owned`/`borrowed` на параметрах extern-функций; проверка на границе (borrowed не переживает вызов, owned передаёт владение)
- [ ] `forgen inspect --memory <file>`: аннотированный листинг — где что освобождается, какой слой выбран
- [ ] Замыкания: автоматическое определение захватов; захват, переживающий фрейм → объект-захват автоматически в @arc (Layer 3), остальное — Layer 1/2; пользователь ничего не пишет
- [ ] Тесты: циклический граф A→B→A с @weak (утечки нет), FFI owned/borrowed (нарушение = ошибка компиляции), AutoFree-порядок, @arc многопоточный (гонки — loom-style стресс на C-рантайме)
- [ ] Замер накладных расходов: @arc-дереференс vs raw (таблица)

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

## M8. Performance Sprint `[ ]`

- [ ] PGO production: `--pgo-train` сбор профиля (база в src/pgo.rs есть) → `--pgo-use`: LLVM — реальные PGO metadata; Cranelift — branch-probability hints + hot/cold split
- [ ] Замеры PGO: vec/matmul/echo с PGO vs без (цель +5–15% на ветвистом коде); детерминизм результата с PGO == без
- [ ] LTO: thin-LTO по умолчанию в release-пути LLVM (fallback без lld); замер размера+скорости
- [ ] Codegen-аудит: comdat/section placement, const-pool шэринг, регистровое давление (по Clif-дампу); дедупликация строковых литералов в data-секции
- [ ] Размер бинарников: суммарный размер всех examples до/после; честная цель tiny-hello ≤ 40 КБ (если недостижимо с C-рантаймом — задокументировать потолок и причины)
- [ ] Скорость компилятора: профилировка после M4, следующая тройка горячих мест; цель 200k строк полный build < 60 c
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
- [ ] Вывод типа результата метода stdlib-класса без аннотации: `val parts = StringUtils.split(...)` не даёт `parts` тип List<Str> в lowering (lookup_var_type пуст), элемент печатается как число. С аннотацией работает. Найдено 2026-09-25 (M1 text-срез); фикс в M4/M7 (infer для class-receiver method calls).
- [ ] РОСТ MAP ЛОМАЕТСЯ НА ≥513 ЗАПИСЯХ (пре-существующий, есть на HEAD 7fc446a): `while i < 513 { m.insert(fmt"key_{i}", i*3) }` → `m.len()` = 512 (заморожен), дальнейшие вставки теряются; при этом m.get("key_512") НАХОДИТ значение — противоречие между map[0] и содержимым. C-харнесс тех же вызовов (gcc, tests-вне-пайплайна) корректен: len=600 на 600 вставках. Точка ЛОМАЕТСЯ между cap 512→1024; трасса роста показывает old_cap=0/мусор в hdr->capacity при валидном magic — подозрение на pool-взаимодействие или слот-дисциплину map-переменных в бэкенде (AssignVar rebinding после роста). Найдено 2026-09-25 (M1/HashMap-стресс). Чинить в M4 (Large-Program Stability) — блокирует полный стресс HashMap.
- [ ] `map.get(k) or X` / `?` на map.get — E0944 при codegen: чекер типизирует map.get как Outcome<T>, но рантайм datara_rt_map_get возвращает сырое значение (ABI-разнобой). Фикс по образцу list pop v1.4.1: datara_rt_map_get_outcome, возвращающий Outcome-объект. Найдено 2026-09-25 (M0/T1-матрица); чинить в M1 (stdlib.collections) или M4.
- [ ] `?`-распаковка Outcome<List<T>>: payload_repr='List' теряет тип элемента → `l[0]?` на List<Outcome<Int>> даёт E0944 (индексный элемент не получает класс в val_to_class). Найдено 2026-09-25; фикс — протаскивать полный GenericInstance в PropagationSite.payload_repr и var_to_class; фаза M4/M7 (infer).
- [ ] match по enum-параметру с `fmt"..."` в армах → E0944 `__tag` (класс enum теряется в val_to_class, когда тело армы содержит FormatStr; с константными Str-армами тот же match работает, без main — падает даже с Int-армами). Найдено 2026-09-25 при написании TUTORIAL Step 6b; фикс в M4/M7.
- [ ] `Map.new()` — статический конструктор дженерик-класса из stdlib-модуля резолвится в Unit/undefined; канон сейчас — литерал `{ }`. Найдено 2026-09-25; фикс в M1 (конструкторы коллекций).
- [ ] Русские комментарии в .dtr-файлах читаются кракозябрами при некоторых путях чтения (кодировка; низкий приоритет, фикс в M14 при ревизии examples)
- [ ] tmp_*-файлы в корне — чистить перед релизом (M14 чеклист)

## 6. Протокол выполнения

1. Одна фаза за раз; внутри фазы пункты сверху вниз.
2. Каждый `[x]` сопровождается: дата + команда + ключевой вывод (одна строка).
3. Никаких «должно работать» — только воспроизведённый вывод.
4. Замеры честные: если цифра хуже цели — записываем как есть и разбираем, почему.
5. Изменения публичных интерфейсов (AST/IR/CLI) в конце фазы сверять с migration-заметками (M14 соберёт).
6. Перед стартом фазы: `git status` чистый, гейт зелёный.