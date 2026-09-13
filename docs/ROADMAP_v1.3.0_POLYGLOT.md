# ROADMAP v1.3.0 — Datara Universal Polyglot Fabric (C#, Zig, Lua, Python, Java)

## Цель релиза v1.3.0
Создание бесшовной аппаратной полиглот-матрицы с нулевыми накладными расходами (Zero-Cost Polyglot Fabric).

---

## 1. Ключевые столпы архитектуры v1.3.0

### Столп 1: Zero-Copy Memory Plane (Унифицированная память)
- Единый контракт среза памяти в RAM: `{ ptr: *mut u8, len: usize, capacity: usize, stride: usize }`.
- Нулевое копирование данных между Datara `List<T>`, NumPy / PyTorch `ndarray`, C# `ReadOnlySpan<T>` и Zig `[]const T`.

### Столп 2: Direct Polyglot Call Syntax (Прямой синтаксис)
- Отказ от строкового `eval`:
  ```datara
  use python.numpy as np
  use csharp.GodotPhysics as gphys
  use zig.simd_math as zm

  fn main() {
      let a: Float = np.sin(1.57)             // Прямой вызов Python-функции
      let b: Float = zm.dot_product(v1, v2)   // Прямой вызов скомпилированного Zig
      let c: Bool  = gphys.check_aabb(box1)   // Прямой вызов C# Native AOT
  }
  ```

### Столп 3: Inline Polyglot Blocks (`inline`)
- Встраивание блоков кода чужих языков прямо в `.dtr`:
  ```datara
  zig inline {
      export fn avx512_accumulate(buf: [*]const f32, count: usize) f32 {
          // Нативный ультрабыстрый код на Zig
      }
  }
  ```

---

## 2. Мосты и экосистемы в v1.3.0

1. **C# / .NET Bridge (Unity, Godot, Avalonia, WPF)**:
   - `forgen export csharp` для создания высокопроизводительных нативных плагинов без пауз сборщика мусора (GC).
   - Поддержка C# Native AOT (`PublishAot=true`).
2. **Zig Bridge (Zero-Dependency C/C++ замена)**:
   - Прямая стыковка по C ABI без накладных расходов.
   - Использование тулчейна Zig для кросс-компиляции C/C++ зависимостей.
3. **Lua / LuaJIT Bridge (Геймдев-моды и скриптинг)**:
   - Встраиваемый рантайм LuaJIT (200 КБ) для написания модов и сценариев прямо в играх на Datara.
4. **Java / JVM Bridge via Project Panama**:
   - Современный Foreign Function & Memory API (Java 22+) без тяжелого JNI.
5. **Модули 2.0 & Local Sparks**:
   - Поддержка `module SubKit { pub fn ... }` внутри одного файла.
   - Локальные упакованные библиотеки `.spark`.
