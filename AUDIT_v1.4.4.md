# Полный аудит языка Datara / компилятора forgen v1.4.4

Дата: 19.09.2026. Метод: чтение исходников (lexer, parser, types, dmir, codegen) + живые проверки `forgen check/run` на синтетических .dtr файлах + `cargo test --lib` (49/49 ok).

## Общая оценка

**7.5 / 10** — для домашнего/раннего языка это очень сильный результат: честный SSA-мидлэнд (DMIR) с Evidence Gate, три бэкенда (Cranelift JIT, LLVM AOT, WASM), аффинное владение без аннотаций, fail-closed диагностика с explain-системой (E0100 про `class` — образцовый текст ошибки), `?`/`or` вместо исключений. Тесты проходят, компилятор быстрый.

Главные проблемы — не в архитектуре, а в **гигиене поверхности языка**: синонимы, недоделанные/мёртвые ключевые слова, размазанная таблица алиасов функций. Это то, что убьёт язык при первом внешнем пользователе: нельзя выучить язык, у которого 4 способа написать одно и то же, и никто не знает, какой канонический.

---

## Категория A: Синонимы (то, что надо СХЛОПНУТЬ до одной формы)

### A1. Типы: разброс написания (худшая находка)

`src/types/resolve.rs:139-145` — таблица принимает **всё подряд**:

| Канонический | Мусорные синонимы (сейчас всё валидно) |
|---|---|
| `Str` | `String`, `str`, `string` |
| `Int` | `Int64/32/16/8`, `i64/i32/i16/i8`, `isize/usize`, `u64/u32/u16/u8/u128/i128`, `USize`, `Byte`, `byte` |
| `Float` | `Float64/Float32`, `f64/f32/f16` |
| `dec64` | `Dec64` (Display печатает `dec64` с маленькой буквы — единственный тип не в PascalCase!) |
| `float4` | `F32x4`, `Float4`, `simd_f32x4` |
| `int4` | `I32x4`, `Int4`, `simd_i32x4` |

Причём проверкой подтверждено: `let s: String = "x"` — компилируется и работает. `str`/`string` — тоже принимаются парсером как имена типов без ошибки «unknown type» (тихо уезжают в `Class("string")` и падают позже с малопонятной E-OUT-001). README обещает и `Int64`, и `Int`, и байтовые типы, но внутри компилятора **все они — один и тот же `DataraType::Int` (i64)**: `Int8` — это ложь. `let x: Int8 = 127 + 1` даёт 128 без переполнения — типа Int8 не существует, только его имя.

**Предложение:** канонический набор: `Int, UInt, Float, Dec64, Dec128, Bool, Str, Char, Unit, Never, Val, RawPtr, float4, int4`. Все остальные написания:
1. Парсер пишет warning `W-TYPE-ALIAS` («`i64` is a legacy alias for `Int`»).
2. Через релиз → error E-TYPE-ALIAS с подсказкой канона.
3. Ширины (Int8/16/32 и знаковость) — либо реально реализовать с отдельным DataraType и переполнением, либо честно удалить из документации. Оставлять фиктивные имена — худший вариант.
4. `Dec64` в Display заменить на `Dec64` (PascalCase), регистронезависимость убрать.

### A2. Ключевые слова-дубликаты в лексере (`src/lexer/mod.rs:845-940`)

| Пара | Статус | Что делать |
|---|---|---|
| `fn` / `function` | оба валидны | Оставить `fn`. `function` → E0100-стиль error («`function` does not exist, use `fn`»). |
| `struct` / `record` / `entity` / `class` | все ведут в один `parse_class_decl` | Канон — `struct`. `class` уже выдаёт отличный E0100. То же самое сделать для `record` и `entity` (сейчас оба молча работают). |
| `match` / `decide` / `select` | три конструкции | См. A3 — это НЕ полные синонимы, но 2 из 3 лишние. |
| `use` / `import` / `using` | `import` → use-decl, `using` → и use-decl и composition (`using Component`) | Канон: `use` для импортов, `using` ТОЛЬКО для композиции. `import` → error с подсказкой. |
| `pub` / `export` | оба = is_export | Канон: `pub` (согласовано с `pub fn` в README). `export` → warning → error. |
| `try`/`catch` | уже удалены с отличным текстом ошибки | ОК, но токены Try/Catch, Stmt::TryCatch и вся ветка lowering (`src/dmir/lowering/decl.rs:564`, `stmt.rs:896`) — мёртвый код, который всё ещё компилируется и парсится в `decl.rs:507-508`. Вычистить. |
| `task` / `flow` / `process` / `packet` / `entity` / `role` / `component` | `Decl::Flow` и `Decl::Task` — это `FunctionDecl`, и **вся цепочка (checker, lowering, codegen) обрабатывает их идентично** `Decl::Function` (`src/types/check/decl.rs:92`, `src/dmir/lowering/mod.rs:781`) | Это фантомные «фичи». Либо реализовать отличающуюся семантику (тогда в бэкенде должен быть отдельный путь — его нет), либо убрать ключевые слова. `async fn` существует и работает — этого достаточно для v1. |

### A3. `decide` vs `select` — полные синонимы (подтверждено)

`src/parser/expr.rs`: `parse_decide_expr` (строка 679) и `parse_select_expr` (строка 768) — **посимвольно идентичный код**, оба строят условные ветки `cond => body, else => body`, дают одинаковый AST-вариант (Decide → свой, Select → свой), но lowering их обрабатывает одинаково (`expr_composite.rs:390` и `:464` — тот же код). Живая проверка: оба возвращают одинаковый результат.

- `if/else` — это тоже то же самое (`parse_if_expr` строит `Expr::Select` из `SelectArm`!).
- `match` — единственная действительно отличная конструкция (паттерны, exhaustive matching).

**Предложение:** оставить `if/else` (императивный стиль) и `match` (паттерны). `decide` и `select` — удалить (сначала deprecation warning + автосug в ошибке, потом error). Если хочется оставить `decide` как «выражение-переключатель без паттернов» — тогда удалить `select`, а `decide` должен отличаться семантикой (например, бранчлесс/select-инструкция), иначе это просто второе имя.

### A4. Алиасы функций в prelude (`src/types/prelude.rs`, ~1690 строк)

Каждая функция зарегистрирована в 3-5 написаниях:
- `str_substring` / `substring` / `substr` / `str_substr` / `datara_rt_str_substring`
- `str_repeat` / `repeat` / `datara_rt_str_repeat`
- `str_pad_left` / `pad_left` / ...; `str_replace` / `replace`; `to_upper`/`to_lower`/`split`/`join`
- `file_read` / `read_file`; `file_write` / `write_file`
- `len` / `length` / `str_len` / `byte_len`
- `arena_alloc` / `mem_alloc` / `stack_alloc` — **три разных имени для одного рантайм-символа** (это задокументировано в коде, но это не оправдание)
- Плюс вся таблица продублирована: блок «4-Tier Memory» (строка ~1100+) повторно вставляет те же `arena_alloc`, `ptr_read_i64` и т.д., которые уже вставлены выше. `str_len`, `byte_len`, `str_chars`, `char_len`, `str_byte_at`, `str_sanitize_utf8`, `validate_utf8` вставлены дважды.

**Предложение:** канон = префиксная форма (`str_...`, `file_...`, `math_...`) + методы через UFCS (`.repeat(n)`, `.pad_left(...)`) там, где они уже есть. Все голые алиасы (`repeat`, `replace`, `split`, `join`, `substr`, `substring`) → удалить с W→E миграцией и lint-подсказкой. Дедуп: строить таблицу из одной константы `const PRELUDE_SIGNATURES: &[(&str, &[DataraType], DataraType)]` вместо 1690 строк HashMap-вставок (и это же станет источником для LSP-автодополнения — сейчас lsp не знает про prelude-функции).

### A5. Методы List: `push` / `append` / `add`

Задокументировано в `src/types/prelude.rs:1-12`: «canonical push, append — explicit alias, add — accepted third spelling». Три написания одного — плохо. Оставить `push`, остальные → warning → error.

---

## Категория B: НЕ трогать (разные вещи, похожие названия — это осознанный дизайн)

По запросу: «разные конструкции, которые отвечают за разные вещи, не трогать». Проверил — вот они:

1. **`out` / `err`** — операторы вывода в stdout/stderr. Разные потоки. Не трогать.
2. **`let` / `mut` / `val` / `mut val`** — Variable Triad. Семантически различны (immutable static / type-locked mutable / gradual container / dynamic). Это фича, не дубликат. Не трогать.
3. **`&&` vs `&`, `||` vs `|`** — логические (Bool) vs битовые (Int). Строгая типизация делает их непересекающимися. Не трогать.
4. **`match`** — паттерны + exhaustiveness; это не «if с другим именем». Не трогать (а `decide`/`select` — да, трогать, см. A3).
5. **`T?` / `Option<T>`** и **`T!E` / `Result<T,E>` / `Outcome<T>`** — синтаксические формы одного, это ОК (это sugar, а не синоним-мусор). Но внутреннее представление через `Maybe<T>`/`Outcome<T>` как реальных классов (`PropagationKind::flag_field` с хардкодом `"is_success"`/`"is_some"`) — хрупко; см. C3.
6. **`with`** (RAII-ресурс) — уникален. Не трогать.
7. **`behavior` / `impl` / `role`** — три механизма расширения. `behavior Type { }` и `impl Trait for Type { }` действительно разные (внешнее поведение vs trait-реализация). Оставить. Но у `behavior` метода есть `replaces` (замена метода) — это нормальная фича.
8. **`unsafe(justification: "...")`** — уникальная и хорошая фича. Не трогать.
9. **`comptime`** — уникален. Не трогать.
10. **`fmt"..."` vs `"..."`** — интерполяция только по префиксу. Это отличный дизайн (JSON/regex не ломаются). Не трогать.

---

## Категория C: Баги и плохие решения (то, что мне не нравится)

### C1. 🔴 `true == 1` компилируется (проверено, exit 0)

`src/types/check/binary.rs:421-435`: комментарий честно говорит «Bool/Int cross-comparisons are intended dynamic behavior (truthy scalars)». Но это противоречит собственной доктрине языка: AGENTS.md и SPEC_V1 Gate 7 запрещают truthy-целые, `if a` с Int уже даёт E-TYPE-001. Получается: условие строгое, сравнение — нет. `true == 1` → true — это источник тихих багов.

**Фикс:** убрать Bool из `is_truthy_scalar`, добавить Bool↔Int в отчёт `report_bad_operands` с подсказкой «compare with `(x != 0)` explicitly».

### C2. 🔴 `dec64` — мёртвый тип (проверено)

`let d: dec64 = 1.5` → E-TYPE-001 «expected dec64, got Float». Нет dec-литералов в лексере, нет пути Float→Dec64, prelude не содержит ни одной dec-функции. README продаёт `Dec64` как «exact financial decimal» — это ложь на уровне README. То же с `Dec128`.

**Фикс:** либо реализовать (литералы с суффиксом `1.5d`, арифметика через runtime-библиотеку, рантайм-символы), либо убрать из resolve.rs/README до реализации. Вариант-минимум: оставить только type alias `type Dec64 = Float` с честным предупреждением, что это бин. флоат.

### C3. 🔴 Связка типов и рантайма через строковые хардкоды

`PropagationKind::flag_field()` возвращает `"is_success"`/`"is_some"` — lowering привязан к полям stdlib-классов по имени строки. `is_pod()` содержит список `"Str" | "String" | "List" | "Map" | "Outcome" | "Maybe"`. `DataraType::String` внутренне называется `String`, печатается как `Str`, а вrepr-строках встречаются оба. Это же касается `Result`/`Outcome`-разрешения в `resolve.rs:75-84` ( Outcome<T> с одним аргументом тихо становится Result<T, String>).

**Фикс:** ввести один канонический enum `RuntimeRepr` вместо строк; `Outcome<T>` переименовать концептуально в единственный `Result<T>` (один тип ошибки = `Str`, отдельный `Result<T,E>` — удалить или сделать сахаром). Сейчас `Outcome`, `Result`, `T!E` — три имени одного, и `resolve.rs` склеивает их костылём.

### C4. 🟡 `or` конфликтует: keyword vs функция

`or` — это OrKeyword (fallback `x or { ... }`), но prelude регистрирует `or` как бит-функцию `or(Int, Int)` (A4). Проверка: `out or(1,2)` → E-SYNTAX-001 «Unexpected token: OrKeyword». То есть алиас зарегистрирован, но синтаксически недостижим — мёртвая регистрация, вводящая в заблуждение. То же для `and`/`xor` как функций: `and`, `or`, `xor` должны быть только операторами `&`, `|`, `^`. Удалить эти записи из prelude, оставить `math_or` и т.п. — или лучше вообще ничего, операторы уже есть.

### C5. 🟡 Нет диагностик на неизвестные типы

`let s: str = "x"` не даёт «unknown type 'str'». Имя уезжает в `Class("str")` и ломается далеко от источника (E-OUT-001 про Display). **Фикс:** в `resolve_type_node_in`, если имя не резолвится ни в примитив, ни в class/alias/trait/type-param — diag.error «unknown type», + did-you-mean по редакционному расстоянию.

### C6. 🟡 Мусор в репозитории

~200 .exe, .obj, .bc, .ll, .lib файлов в корне — артефакты прошлых прогонов, замусоривают листинг и git. `git ls-files | grep -c .exe` = 0 (не закоммичены — хорошо), но рабочая директория должна чиститься, а `.gitignore` — дополняться (`*.exe`, `*.obj`, `*.bc`, `*.ll`, `*.lib`).

### C7. 🟡 README vs реальность

- README таблица типов обещает `Int8..Int64`, `UInt*`, `Float32`, `Dec64/Dec128`, `String` и `Str` как разные вещи — а компилятор всё это сворачивает в Int/Float/String (см. A1, C2).
- README описывает `parallel for`, `float4`, `asm`,capability-модель — работают. Но `try/catch` и `class` в таблице ключевых слов vscode-расширения (`editors/vscode/syntaxes/datara.tmLanguage.json`) всё ещё подсвечиваются как валидные — синхронизировать tmLanguage с реальным списком после чистки.
- README говорит «33 official Standard Library modules», `ls stdlib` — 22 каталога. Сверить.

### C8. 🟡 Локальные несостыковки

- `std` has `now()`/`now_ms`/`now_ns`/`now_precise_ms` + `datara_rt_now_ns` — полный зоопарк времени, надо канонизировать (`time.now_ms()` namespace) — это отдельная большая работа по stdlib, вне скоупа аудита синтаксиса, но фиксирую.
- `mut-view` — единственное ключевое слово через дефис (HYPHENATED_KEYWORDS), ломает правило «идентификатор без дефисов» и выглядит чужеродно; предложить `mut view` (два слова) или `view mut`.
- В лексере `None` с большой буквы — единственное ключевое слово с заглавной (все остальные lowercase). Согласуется с Option/Some, но тогда `Some` тоже должен быть keyword — сейчас это обычный идентификатор? Проверить; несимметрия — риск.
- `prelude.rs` дублирует вставки (A4) — признак отсутствия единого источника правды.
- `docs/SPEC_V1.md` говорит «Outcome<T> aliased to Result<T, String>» — а resolve.rs принимает `Result<T,E>` с произвольным E. Определиться: E ≠ Str разрешён или нет (сейчас код позволяет, спека нет).

### C9. 🟢 Что сделано хорошо (не трогать, это эталон)

- Тексты ошибок E0100 (`class`), E-SYNTAX-001 (`try/catch`), `:=` — образцовые: точный caret, help, `forgen explain`.
- Лексер: ранее молча проглатывал символы, теперь — hard error с U+кодом (комментарий в коде фиксирует урок).
- Разделение literal/interpolated строк.
- `StrBuf` с lint-подсказкой L1401 на конкатенацию в цикле.
- Cargo check чистый, lib-тесты зелёные.

---

## План фикса (по фазам, с минимальным риском)

### Фаза 0 — гигиена (1-2 дня)
1. Дописать `.gitignore` (`*.exe`, `*.obj`, `*.bc`, `*.ll`, `*.lib`, `.forgen_cache/`, `bench_*.exe` и т.п.), вычистить корень.
2. Удалить двойные вставки в `prelude.rs`, вынести таблицу в `const PRELUDE: &[...]` (без изменения поведения).
3. tmLanguage: убрать `class`, `try`, `catch`, `function` из keyword-листов; добавить `struct`, `comptime`, `unsafe`, `out`, `err`.

### Фаза 1 — синонимы типов (3-5 дней)
4. В `resolve_type_node_in` оставить только канон; для каждого не-канона — warning W-TYPE-0001 c текстом «'X' is a legacy alias for 'Y'».
5. Добавить E-TYPE-0002 «unknown type 'X'» + did-you-mean (C5).
6. Обновить README/README_RU/docs, снести таблицу несуществующих ширин или пометить «planned».
7. `dec64` → `Dec64` в Display, либо убрать до реализации (C2).

### Фаза 2 — ключевые слова (2-3 дня)
8. `record`, `entity`, `function`, `export`, `import` → error E0100-стиля (по образцу `class`).
9. `select` → warning → удалить (A3); `decide` — решить: оставить как отдельную фичу с реальной select-семантикой или удалить тоже.
10. `Flow`/`Task`/`Packet`/`Process`/`Entity`/`Role` decl-варианты: те, что не имеют собственной семантики — удалить из AST и лексера; `Component`/`Role` оставить, только если есть отличающийся lowering (сейчас есть `RoleDecl` с методами — проверить, что role реально отличается от trait; если нет — объединить с trait).
11. Вычистить мёртвый `Try/Catch` (токены, Stmt::TryCatch, ветки lowering, decl.rs:397,507).

### Фаза 3 — prelude-канон (3-4 дня)
12. Голые алиасы (`repeat`, `replace`, `split`, `join`, `substr`, `substring`, `read_file`, `write_file`, `length`) → warning W0200 c автосug, потом удалить.
13. `push`/`append`/`add` → канон `push`.
14. `and`/`or`/`xor`-функции → удалить (C4).
15. Time-функции: `now`, `now_ms`, `now_ns`, `now_precise_ms` → канон `time`-неймспейс или оставить `now_ms/now_ns` и удалить остальные.

### Фаза 4 — семантика (1-2 недели)
16. `true == 1` → error (C1).
17. `Outcome` vs `Result` vs `T!E` — один канон (C3), убрать stringly-typed рантайм-биндинги.
18. `Dec64/Dec128`: реализовать или удалить (C2).
19. `Int8..Int32/UInt*`: реализовать или удалить (A1).
20. `mut-view` → `mut view` (или принять и задокументировать дефис как часть грамматики).

### Фаза 5 — миграция пользователей (инструменты)
21. `forgen fix` (codemod): автоматически переписывает алиасы на канон по таблице соответствия.
22. Каждый удалённый синоним должен иметь: W-фазу (1 релиз) → E-фазу → запись в `docs/GLOSSARY.md` «канон vs удалённые алиасы».
23. CI-gate: тест-фикстура, asserting что лексер/resolve принимают ровно канонический набор — чтобы синонимы не проросли обратно.

## Итоговые цифры
- Уникальных синонимических записей в лексере/типах: ~35 (A1: 25 написаний типов, A2: 6 пар, A4/A5: алиасы функций).
- Из них «реально разных» (не трогать): `out/err`, Variable Triad, `&&`/`&`, `match`, `?`/`or`, behavior/impl/role, with, unsafe, comptime, fmt.
- Мёртвых/фантомных ключевых слов: `record`, `entity`, `function`, `flow`, `task`, `process`, `packet`, `try`, `catch`, `select`, `import` (как отдельный от use).
