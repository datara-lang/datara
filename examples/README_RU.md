# Каталог примеров Datara (v1.3.0)

В данном каталоге собраны эталонные исполняемые примеры программ на языке Datara для компилятора Forgen. Все примеры строго проверены и соответствуют спецификации Datara v1.3.0.

---

## Как запускать примеры

Любой пример можно проверить и запустить с помощью утилиты `forgen`:

```bash
# Проверка типов, эффектов и владения памяти:
forgen check examples/<файл>.dtr

# Нативный запуск (через бэкенд Cranelift/LLVM):
forgen run examples/<файл>.dtr

# Режим быстрой JIT-компиляции:
forgen run --jit examples/<файл>.dtr
```

---

## Каталог примеров

### 1. Основы и управление потоком исполнения
- `01_hello_world.dtr`: Минимальная программа на Datara с точкой входа `fn main()` и выводом строки.
- `01_vertical_slice.dtr`: Вертикальный срез компилятора для тестирования пайплайна.
- `02_math_and_loops.dtr`: Целочисленная арифметика, цикл while и строковая интерполяция (`fmt"..."`).
- `04_decide_and_control.dtr`: Сопоставление с образцом через выражение `decide`.
- `04_enum_adt.dtr`: Алгебраические типы данных (ADT) и перечисления с полезной нагрузкой.
- `05_pipeline_dataflow.dtr`: Конвейерная обработка данных через оператор `|>`.

### 2. Data-Oriented Design (DOD) и пост-ООП архитектура
- `02_class_modern_oop.dtr`: Объявление структур (`struct`) и методов в блоке `behavior`.
- `03_post_oop_class.dtr`: Каноническая пост-ООП модель: плоские непрерывные структуры данных и отделенная логика в блоках `behavior`.
- `03_split_behavior.dtr`: Раздельное объявление методов в блоках `behavior`.
- `18_data_oriented_structs.dtr`: Физическая симуляция частиц с гарантией локальности процессорного кэша и нулевыми накладными расходами на vtable.

### 3. Доказуемый код (PCC) и практические CLI-утилиты
- `06_phase1_complete_app.dtr`: Полноценное модульное приложение с контрактами верификации.
- `07_entity_process_model.dtr`: Модель сущностей и процессов.
- `08_text_analyzer_cli.dtr`: Статистический анализатор читаемости текста (индекс Coleman-Liau) с математическими контрактами безопасности деления (`require != 0`).
- `09_error_propagation_question.dtr`: Обработка ошибок через типы `Outcome` с оператором распространения `?` и запасным значением `or`.
- `09_matrix_math_cli.dtr`: Многомерные матричные вычисления и алгоритмы линейной алгебры.
- `10_database_query_cli.dtr`: Встраиваемый реляционный движок обработки данных с фильтрацией и агрегацией.
- `11_crypto_pow_cli.dtr`: Криптографический Proof-of-Work хэшер и генератор блоков на основе алгоритма Knuth LCG.
- `zero_js_dashboard.dtr`: Нативный реактивный дашборд.

### 4. Градуальная типизация и адаптивная оптимизация Comptime
- `12_dynamic_variables_val.dtr`: Триада переменных Datara (`let`, `mut`, `val`, `mut val`) для динамической и статической типизации.
- `19_adaptive_flow_typing.dtr`: Адаптивный анализ потока типов (SSA Register Specialization), устраняющий накладные расходы упаковки (boxing) динамических переменных.
- `dynamic_guarded_demo.dtr`: Динамические вычисления с проверкой прав доступа.

### 5. Универсальный полиглотный движок с нулевой задержкой (Новинка v1.3.0)
- `13_polyglot_zig_math.dtr`: Прямой вызов Zig SIMD-вычислений и нативных функций без накладных расходов FFI (`zig_eval_int`, `zig_call`).
- `14_polyglot_csharp_nativeaot.dtr`: Внутрипроцессный вызов скомпилированных C# / .NET NativeAOT модулей (`csharp_invoke_i64`).
- `15_polyglot_lua_scripting.dtr`: Встраиваемый скриптинг на Lua / LuaJIT с горячей перезагрузкой (`lua_eval_int`, `lua_exec`).
- `16_polyglot_python_zerocopy.dtr`: Бесшовный мост к экосистеме Python (NumPy, SciPy) с разделяемой памятью (`py.exec`, `py.eval_int`, `py.eval_float`).
- `17_polyglot_parallel_computing.dtr`: Многопоточное параллельное исполнение задач на Zig, Lua и C# NativeAOT (`polyglot_parallel_exec`).
- `20_polyglot_python_universal.dtr`: Внутрипроцессный вызов любых произвольных библиотек Python (requests, hashlib, json, platform) с нулевой задержкой.

### 6. Синхронизация зависимостей (`requirements.txt`)
- `requirements.txt`: Список сторонних Python-зависимостей проекта. Для автоматической установки через `pip` используйте команду `forgen install-deps`.

---

## Проверка всех примеров

Все примеры можно одновременно протестировать одной командой:
```powershell
Get-ChildItem -Path examples -Filter *.dtr | ForEach-Object { forgen run $_.FullName }
```
