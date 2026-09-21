use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorCode {
    // Syntax Errors (E-SYNTAX-*)
    SyntaxUnexpectedToken,
    SyntaxUnterminatedString,
    SyntaxUnterminatedComment,
    SyntaxInvalidNumber,
    SyntaxInvalidChar,
    SyntaxExpectedExpression,
    SyntaxExpectedIdentifier,
    SyntaxExpectedType,
    SyntaxInvalidEscape,
    RecursionLimitExceeded,

    // Resolution Errors (E-RESOLVE-*)
    ResolveUndefinedSymbol,
    ResolveDuplicateSymbol,
    ResolveUnknownType,
    ResolveCircularDependency,
    ResolveUnreachableModule,
    PrivateItemAccess,

    // Type Errors (E-TYPE-*)
    TypeMismatch,
    TypeCannotInfer,
    TypeMissingReturn,
    TypeInvalidBinaryOp,
    TypeInvalidUnaryOp,
    TypeInvalidMemberAccess,
    TypeGenericMismatch,
    TypeIncomparableOperands,
    TypeUnknownMethod,
    // v1.4.1: `?` used inside a function whose return type cannot receive
    // the propagated error (not Outcome/Result/Maybe-shaped).
    QuestionPropagation,

    // Borrow & Ownership Errors (E-BORROW-*)
    BorrowUseAfterMove,
    BorrowCannotMutateImmutable,
    BorrowConflictActiveView,
    BorrowMultipleMutableViews,
    BorrowEscapingView,
    BorrowConflict,

    // Effect Errors (E-EFFECT-*)
    EffectImpureInPureContext,
    EffectUnsafeOperation,
    EffectUnhandledIO,

    // Codegen Errors (E-CODEGEN-*)
    CodegenBackendFailed,
    CodegenLinkerFailed,
    CodegenIOError,

    // Security & Zero-Trust Gate Errors (E0940-E0943)
    SecurityViolation,
    ProofCarryingCodeViolation,
    UncheckedFFIViolation,
    DataRaceViolation,

    // Real-Time & Safety Verification Gate Errors (E0950-E0951)
    AllocationViolation,
    PanicViolation,

    // Polyglot & C Import Errors (E0960-E0962)
    CImportHeaderNotFound,
    CImportReadFailed,
    CImportUnsupportedConstruct,

    // Language Safety & Formal Verification Errors (E0310-E0947)
    NonExhaustiveMatch,
    UnreachablePattern,
    LoopControlOutsideLoop,
    DimensionMismatch,
    InvariantViolation,
    TerminationViolation,
    RangeViolation,
    ContractViolation,

    // Machine-readable Codegen & Internal Errors (E0901-E0904)
    InternalVerification,
    InlineAsmUnsupported,
    UnknownBinop,
    CApiArgLimitExceeded,
    AsyncBackendUnsupported,

    // Cross-compilation Errors (E0980)
    CrossCompilationMissingToolchain,

    // Inline Attribute Errors (E0956-E0957, v1.3.4)
    InlineAlwaysExtern,
    InlineAttributeInvalid,

    // Allocator Tier Errors (E1401, E1406, v1.4.0)
    ArenaEscape,
    PoolCapacityExceeded,

    // Structured Inline Assembly Errors (E1402-E1405, v1.4.0)
    AsmUnsupportedInst,
    AsmOperandNotInt,
    AsmRequiresUnsafe,
    AsmUnsupportedBackend,

    // Optimizer Correctness Warnings (E-OPT-*)
    OptimizationUnproven,

    // Deprecation & Modernization Warnings (W0100)
    DeprecatedFeature,

    // Canonical-Form Warnings (W-SYN-*): the language accepts a legacy or
    // synonymous spelling, but the canonical keyword is something else.
    // These are warnings (not errors) so existing code keeps compiling
    // while nudging codebases toward one spelling per construct.
    CanonicalKeyword,
    CanonicalTypeSpelling,
    BoolIntComparison,

    // Bridge Block Errors (E-BRIDGE-*, v1.4.2)
    BridgeTypeMismatch,
    BridgeUnsupportedType,
    BridgeUnknownLanguage,

    // Capability 2.0 Errors (E-CAP-*, v1.4.2)
    CapMissingPermission,
    CapGlobViolation,

    // Entity-Guided Evidence Optimization Errors (Datara v1.5.0)
    ClassKeywordForbidden,
    StructMethodForbidden,
    ComponentMethodForbidden,
    ComponentNonPodField,
    BehaviorFieldForbidden,
    BehaviorImpure,
    RoleCapabilityMissing,
    ImplCoherenceViolation,
    ImplMissingSuperTrait,
    UnprintableType,
    DeprecatedPrintFunction,

    // Comptime Errors (E-CT-*, v1.4.5)
    ComptimeRecursionLimit,
    ComptimeForbiddenEffect,
    ComptimeNonConstArg,

    // Bare-Metal & Target Errors (v1.4.5)
    MmioRequiresUnsafe,
    TargetRequiresLlvm,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::SyntaxUnexpectedToken => "E-SYNTAX-001",
            ErrorCode::SyntaxUnterminatedString => "E-SYNTAX-002",
            ErrorCode::SyntaxUnterminatedComment => "E-SYNTAX-003",
            ErrorCode::SyntaxInvalidNumber => "E-SYNTAX-004",
            ErrorCode::SyntaxInvalidChar => "E-SYNTAX-005",
            ErrorCode::SyntaxExpectedExpression => "E-SYNTAX-006",
            ErrorCode::SyntaxExpectedIdentifier => "E-SYNTAX-007",
            ErrorCode::SyntaxExpectedType => "E-SYNTAX-008",
            ErrorCode::SyntaxInvalidEscape => "E-SYNTAX-009",
            ErrorCode::RecursionLimitExceeded => "E0105",

            ErrorCode::ResolveUndefinedSymbol => "E-RESOLVE-001",
            ErrorCode::ResolveDuplicateSymbol => "E-RESOLVE-002",
            ErrorCode::ResolveUnknownType => "E-RESOLVE-003",
            ErrorCode::ResolveCircularDependency => "E-RESOLVE-004",
            ErrorCode::ResolveUnreachableModule => "E-RESOLVE-005",
            ErrorCode::PrivateItemAccess => "E0042",

            ErrorCode::TypeMismatch => "E-TYPE-001",
            ErrorCode::TypeCannotInfer => "E-TYPE-002",
            ErrorCode::TypeMissingReturn => "E-TYPE-003",
            ErrorCode::TypeInvalidBinaryOp => "E-TYPE-004",
            ErrorCode::TypeInvalidUnaryOp => "E-TYPE-005",
            ErrorCode::TypeInvalidMemberAccess => "E-TYPE-006",
            ErrorCode::TypeGenericMismatch => "E-TYPE-007",
            ErrorCode::TypeIncomparableOperands => "E-TYPE-008",
            ErrorCode::TypeUnknownMethod => "E-TYPE-009",
            ErrorCode::QuestionPropagation => "E-TYPE-010",

            ErrorCode::BorrowUseAfterMove => "E-BORROW-001",
            ErrorCode::BorrowCannotMutateImmutable => "E-BORROW-002",
            ErrorCode::BorrowConflictActiveView => "E-BORROW-003",
            ErrorCode::BorrowMultipleMutableViews => "E-BORROW-004",
            ErrorCode::BorrowEscapingView => "E-BORROW-005",
            ErrorCode::BorrowConflict => "E-BORROW-006",

            ErrorCode::EffectImpureInPureContext => "E-EFFECT-001",
            ErrorCode::EffectUnsafeOperation => "E-EFFECT-002",
            ErrorCode::EffectUnhandledIO => "E-EFFECT-003",

            ErrorCode::CodegenBackendFailed => "E-CODEGEN-001",
            ErrorCode::CodegenLinkerFailed => "E-CODEGEN-002",
            ErrorCode::CodegenIOError => "E-CODEGEN-003",

            ErrorCode::SecurityViolation => "E0940",
            ErrorCode::ProofCarryingCodeViolation => "E0941",
            ErrorCode::UncheckedFFIViolation => "E0942",
            ErrorCode::DataRaceViolation => "E0943",
            ErrorCode::AllocationViolation => "E0950",
            ErrorCode::PanicViolation => "E0951",
            ErrorCode::CImportHeaderNotFound => "E0960",
            ErrorCode::CImportReadFailed => "E0961",
            ErrorCode::CImportUnsupportedConstruct => "E0962",
            ErrorCode::NonExhaustiveMatch => "E0310",
            ErrorCode::UnreachablePattern => "E0311",
            ErrorCode::LoopControlOutsideLoop => "E0312",
            ErrorCode::DimensionMismatch => "E0420",
            ErrorCode::InvariantViolation => "E0945",
            ErrorCode::TerminationViolation => "E0946",
            ErrorCode::RangeViolation => "E0947",
            ErrorCode::ContractViolation => "E0948",
            ErrorCode::InternalVerification => "E0901",
            ErrorCode::InlineAsmUnsupported => "E0902",
            ErrorCode::UnknownBinop => "E0903",
            ErrorCode::CApiArgLimitExceeded => "E0904",
            ErrorCode::AsyncBackendUnsupported => "E0955",
            ErrorCode::CrossCompilationMissingToolchain => "E0980",
            ErrorCode::InlineAlwaysExtern => "E0956",
            ErrorCode::InlineAttributeInvalid => "E0957",
            ErrorCode::ArenaEscape => "E1401",
            ErrorCode::AsmUnsupportedInst => "E1402",
            ErrorCode::AsmOperandNotInt => "E1403",
            ErrorCode::AsmRequiresUnsafe => "E1404",
            ErrorCode::AsmUnsupportedBackend => "E1405",
            ErrorCode::PoolCapacityExceeded => "E1406",
            ErrorCode::OptimizationUnproven => "E-OPT-001",
            ErrorCode::DeprecatedFeature => "W0100",
            ErrorCode::CanonicalKeyword => "W-SYN-001",
            ErrorCode::CanonicalTypeSpelling => "W-SYN-002",
            ErrorCode::BoolIntComparison => "W-TYPE-002",
            ErrorCode::BridgeTypeMismatch => "E-BRIDGE-001",
            ErrorCode::BridgeUnsupportedType => "E-BRIDGE-002",
            ErrorCode::BridgeUnknownLanguage => "E-BRIDGE-003",
            ErrorCode::CapMissingPermission => "E-CAP-001",
            ErrorCode::CapGlobViolation => "E-CAP-002",
            ErrorCode::ClassKeywordForbidden => "E0100",
            ErrorCode::StructMethodForbidden => "E-STRUCT-001",
            ErrorCode::ComponentMethodForbidden => "E-COMP-001",
            ErrorCode::ComponentNonPodField => "E-COMP-002",
            ErrorCode::BehaviorFieldForbidden => "E-BEH-001",
            ErrorCode::BehaviorImpure => "E-BEH-002",
            ErrorCode::RoleCapabilityMissing => "E-ROLE-001",
            ErrorCode::ImplCoherenceViolation => "E-IMPL-001",
            ErrorCode::ImplMissingSuperTrait => "E-IMPL-002",
            ErrorCode::UnprintableType => "E-OUT-001",
            ErrorCode::DeprecatedPrintFunction => "W0102",

            ErrorCode::ComptimeRecursionLimit => "E-CT-001",
            ErrorCode::ComptimeForbiddenEffect => "E-CT-002",
            ErrorCode::ComptimeNonConstArg => "E-CT-003",
            ErrorCode::MmioRequiresUnsafe => "E-MMIO-001",
            ErrorCode::TargetRequiresLlvm => "E-TARGET-001",
        }
    }

    pub fn description(&self, locale: &str) -> &'static str {
        if locale == "ru" {
            match self {
                ErrorCode::SyntaxUnexpectedToken => "Неожиданный токен",
                ErrorCode::SyntaxUnterminatedString => "Незакрытая строковая константа",
                ErrorCode::SyntaxUnterminatedComment => "Незакрытый многострочный комментарий",
                ErrorCode::SyntaxInvalidNumber => "Некорректный числовой литерал",
                ErrorCode::SyntaxInvalidChar => "Недопустимый символ",
                ErrorCode::SyntaxExpectedExpression => "Ожидалось выражение",
                ErrorCode::SyntaxExpectedIdentifier => "Ожидался идентификатор",
                ErrorCode::SyntaxExpectedType => "Ожидалось имя типа",
                ErrorCode::SyntaxInvalidEscape => {
                    "Некорректная escape-последовательность в строковом литерале"
                }
                ErrorCode::RecursionLimitExceeded => {
                    "Превышен лимит глубины вложенности выражений (64)"
                }

                ErrorCode::ResolveUndefinedSymbol => "Неопределённый символ",
                ErrorCode::ResolveDuplicateSymbol => "Дублирующееся объявление символа",
                ErrorCode::ResolveUnknownType => "Неизвестный тип данных",
                ErrorCode::ResolveCircularDependency => "Циклическая зависимость",
                ErrorCode::ResolveUnreachableModule => "Недостижимый модуль",
                ErrorCode::PrivateItemAccess => {
                    "Попытка доступа к приватному элементу другого модуля"
                }

                ErrorCode::TypeMismatch => "Несоответствие типов данных",
                ErrorCode::TypeCannotInfer => "Невозможно вывести тип выражения",
                ErrorCode::TypeMissingReturn => "Отсутствует возвращаемое значение",
                ErrorCode::TypeInvalidBinaryOp => "Недопустимая бинарная операция для типов",
                ErrorCode::TypeInvalidUnaryOp => "Недопустимая унарная операция",
                ErrorCode::TypeInvalidMemberAccess => "Поле или метод не существует в типе",
                ErrorCode::TypeGenericMismatch => "Несоответствие аргументов обобщённого типа",
                ErrorCode::TypeIncomparableOperands => {
                    "Сравнение порядка над несовместимыми типами: неявные преобразования запрещены"
                }
                ErrorCode::TypeUnknownMethod => {
                    "Метод с таким именем не существует для типа получателя"
                }
                ErrorCode::QuestionPropagation => {
                    "Оператор '?' требует, чтобы объемлющая функция возвращала Result/Option-подобный тип"
                }

                ErrorCode::BorrowUseAfterMove => {
                    "Использование значения после перемещения (use-after-move)"
                }
                ErrorCode::BorrowCannotMutateImmutable => {
                    "Попытка изменения неизменяемой переменной"
                }
                ErrorCode::BorrowConflictActiveView => {
                    "Конфликт заимствования: изменение при активном view"
                }
                ErrorCode::BorrowMultipleMutableViews => {
                    "Конфликт заимствования: множественные mut-view запрещены"
                }
                ErrorCode::BorrowEscapingView => {
                    "Утечка заимствования (view не может пережить локальную переменную)"
                }
                ErrorCode::BorrowConflict => {
                    "Конфликт заимствования: нарушение эксклюзивности &mut или чтение при живом &mut"
                }

                ErrorCode::EffectImpureInPureContext => "Побочный эффект в чистом контексте",
                ErrorCode::EffectUnsafeOperation => "Небезопасная операция",
                ErrorCode::EffectUnhandledIO => "Необработанный ввод-вывод",

                ErrorCode::CodegenBackendFailed => "Ошибка генерации нативного кода",
                ErrorCode::CodegenLinkerFailed => "Ошибка компоновщика",
                ErrorCode::CodegenIOError => "Ошибка ввода-вывода при компиляции",

                ErrorCode::SecurityViolation => {
                    "Нарушение безопасности: операция требует мандат полномочий (Capability)"
                }
                ErrorCode::ProofCarryingCodeViolation => {
                    "Нарушение Proof-Carrying Code: операция не имеет доказательства безопасности"
                }
                ErrorCode::UncheckedFFIViolation => {
                    "Небезопасный вызов FFI без блока 'unsafe(justification: ...)'"
                }
                ErrorCode::DataRaceViolation => {
                    "Нарушение параллелизма: потенциальная гонка данных переменной"
                }
                ErrorCode::AllocationViolation => {
                    "Нарушение режима реального времени: динамическое выделение памяти в контексте '@no_alloc'"
                }
                ErrorCode::PanicViolation => {
                    "Нарушение режима реального времени: недоказанный путь паники в контексте '@no_panic'"
                }
                ErrorCode::CImportHeaderNotFound => "Заголовочный файл C не найден",
                ErrorCode::CImportReadFailed => "Не удалось прочитать заголовочный файл C",
                ErrorCode::CImportUnsupportedConstruct => {
                    "Неподдерживаемая конструкция языка C в заголовочном файле"
                }
                ErrorCode::NonExhaustiveMatch => {
                    "Неисчерпывающий паттерн в сопоставлении с образцом (match): покрыты не все варианты"
                }
                ErrorCode::UnreachablePattern => {
                    "Недостижимый паттерн в сопоставлении с образцом (match)"
                }
                ErrorCode::LoopControlOutsideLoop => {
                    "'break' или 'continue' вне тела цикла (while/for/loop)"
                }
                ErrorCode::DimensionMismatch => {
                    "Несоответствие размерностей единиц измерения (Units of Measure)"
                }
                ErrorCode::InvariantViolation => {
                    "Нарушение инварианта структуры данных (Class/Struct Invariant)"
                }
                ErrorCode::TerminationViolation => {
                    "Нарушение завершимости: невозможно доказать завершение цикла или рекурсии в 'pure' функции"
                }
                ErrorCode::RangeViolation => {
                    "Нарушение диапазона: значение выходит за границы допустимого интервала или массива"
                }
                ErrorCode::ContractViolation => {
                    "Нарушение контракта: предусловие (requires) или постусловие (ensures) не выполняется"
                }
                ErrorCode::InternalVerification => {
                    "Внутренняя ошибка верификации промежуточного представления (DMIR)"
                }
                ErrorCode::InlineAsmUnsupported => {
                    "Встроенный ассемблер (Inline Asm) не поддерживается данным целевым бэкендом"
                }
                ErrorCode::UnknownBinop => "Неизвестный бинарный оператор в генераторе кода",
                ErrorCode::CApiArgLimitExceeded => {
                    "Превышен лимит аргументов C API (поддерживается до 8 аргументов)"
                }
                ErrorCode::AsyncBackendUnsupported => {
                    "Асинхронное исполнение (async/await) не поддерживается данным целевым бэкендом (требуется PCS Runtime)"
                }
                ErrorCode::CrossCompilationMissingToolchain => {
                    "Отсутствует инструментарий кросс-компиляции для целевой платформы (требуется lld или кросс-линкер)"
                }
                ErrorCode::InlineAlwaysExtern => {
                    "'@inline(always)' запрещён для extern-объявлений: тело внешней функции недоступно компилятору"
                }
                ErrorCode::InlineAttributeInvalid => {
                    "Некорректный атрибут '@inline': ожидается '@inline', '@inline(always)' или '@inline(never)'"
                }
                ErrorCode::ArenaEscape => {
                    "Значение, размещённое в @arena-функции, покидает область арены: возвращать можно только копируемые типы"
                }
                ErrorCode::AsmUnsupportedInst => {
                    "Инструкция вне поддерживаемого безопасного подмножества asm: доступны mov/add/sub между регистрами, переменными Int и целыми константами"
                }
                ErrorCode::AsmOperandNotInt => "Операнд asm-блока должен иметь тип Int",
                ErrorCode::AsmRequiresUnsafe => {
                    "asm-блок требует обёртки unsafe(justification: \"...\")"
                }
                ErrorCode::AsmUnsupportedBackend => {
                    "Структурированные asm-блоки не поддерживаются этим бэкендом: используйте Cranelift"
                }
                ErrorCode::PoolCapacityExceeded => {
                    "Пул @pool переполнен: количество доказуемо размещаемых значений превышает ёмкость"
                }
                ErrorCode::OptimizationUnproven => {
                    "Трансформация раскладки пропущена: семантическая эквивалентность не доказана"
                }

                ErrorCode::DeprecatedFeature => {
                    "Устаревшая языковая конструкция: используйте современный аналог"
                }
                ErrorCode::CanonicalKeyword => {
                    "Синонимичное ключевое слово: используйте каноническую форму"
                }
                ErrorCode::CanonicalTypeSpelling => {
                    "Синонимичное написание типа: используйте каноническое имя типа"
                }
                ErrorCode::BoolIntComparison => {
                    "Сравнение Bool с целым числом: типы должны совпадать"
                }
                ErrorCode::BridgeTypeMismatch => {
                    "Несоответствие типов аргументов декларативного моста"
                }
                ErrorCode::BridgeUnsupportedType => {
                    "Неподдерживаемый тип данных в сигнатуре декларативного моста"
                }
                ErrorCode::BridgeUnknownLanguage => {
                    "Неизвестный целевой язык для декларативного моста"
                }
                ErrorCode::CapMissingPermission => {
                    "Операция требует объявленного разрешения в секции [capabilities] datara.toml"
                }
                ErrorCode::CapGlobViolation => {
                    "Путь или адрес нарушает разрешённый glob-шаблон в секции [capabilities] datara.toml"
                }
                ErrorCode::ClassKeywordForbidden => {
                    "Ключевое слово 'class' удалено из языка: используйте 'struct' для данных и 'behavior' или 'impl' для методов"
                }
                ErrorCode::StructMethodForbidden => {
                    "Методы внутри 'struct' запрещены: вынесите их в блок 'behavior <Тип>' или 'impl <Трейт> for <Тип>'"
                }
                ErrorCode::ComponentMethodForbidden => {
                    "Компонент является чистым POD-контейнером данных и не может объявлять методы: используйте 'behavior'"
                }
                ErrorCode::ComponentNonPodField => {
                    "Поля компонента обязаны быть POD-типами (Int, Float, Bool, Char или другой компонент): куча запрещена"
                }
                ErrorCode::BehaviorFieldForbidden => {
                    "Блок 'behavior' не может содержать состояние или поля данных: используйте 'struct' или 'component'"
                }
                ErrorCode::BehaviorImpure => {
                    "Методы в 'behavior' чистые по умолчанию: операции с побочными эффектами требуют атрибута #[effect(IO)]"
                }
                ErrorCode::RoleCapabilityMissing => {
                    "Контракт роли требует разрешения, отсутствующего в окружении или [capabilities]"
                }
                ErrorCode::ImplCoherenceViolation => {
                    "Нарушение когерентности: для типа уже существует реализация данного трейта"
                }
                ErrorCode::ImplMissingSuperTrait => {
                    "Отсутствует обязательная реализация супер-трейта, требуемая определением трейта"
                }
                ErrorCode::UnprintableType => {
                    "Тип выражения не является печатаемым: требуется примитивный тип или реализация Display"
                }
                ErrorCode::DeprecatedPrintFunction => {
                    "Использование функций печати устарело: используйте оператор 'out' или 'err'"
                }
                ErrorCode::ComptimeRecursionLimit => {
                    "Превышен лимит глубины рекурсии или шагов вычислений в comptime (E-CT-001)"
                }
                ErrorCode::ComptimeForbiddenEffect => {
                    "Запрещённый побочный эффект в comptime: ввод-вывод, runtime builtins и unsafe запрещены (E-CT-002)"
                }
                ErrorCode::ComptimeNonConstArg => {
                    "Аргументы comptime-функции обязаны быть константами времени компиляции (E-CT-003)"
                }
                ErrorCode::MmioRequiresUnsafe => {
                    "Доступ к MMIO требует блока 'unsafe' или объявления в секции [devices] datara.toml (E-MMIO-001)"
                }
                ErrorCode::TargetRequiresLlvm => {
                    "Целевая архитектура поддерживается только через бэкенд LLVM (E-TARGET-001)"
                }
            }
        } else {
            match self {
                ErrorCode::SyntaxUnexpectedToken => "Unexpected token in source",
                ErrorCode::SyntaxUnterminatedString => "Unterminated string literal",
                ErrorCode::SyntaxUnterminatedComment => "Unterminated multi-line comment",
                ErrorCode::SyntaxInvalidNumber => "Invalid numeric literal format",
                ErrorCode::SyntaxInvalidChar => "Invalid character in input stream",
                ErrorCode::SyntaxInvalidEscape => "Invalid escape sequence in string literal",
                ErrorCode::SyntaxExpectedExpression => "Expected an expression",
                ErrorCode::SyntaxExpectedIdentifier => "Expected an identifier",
                ErrorCode::SyntaxExpectedType => "Expected a type annotation",
                ErrorCode::RecursionLimitExceeded => {
                    "Expression nesting exceeds maximum allowed depth (64)"
                }

                ErrorCode::ResolveUndefinedSymbol => "Undefined symbol reference",
                ErrorCode::ResolveDuplicateSymbol => "Duplicate symbol declaration",
                ErrorCode::ResolveUnknownType => "Unknown type identifier",
                ErrorCode::ResolveCircularDependency => "Circular module dependency detected",
                ErrorCode::ResolveUnreachableModule => "Unreachable module detected",
                ErrorCode::PrivateItemAccess => {
                    "Cannot access private item outside of its declaring module"
                }

                ErrorCode::TypeMismatch => "Static type mismatch",
                ErrorCode::TypeCannotInfer => "Unable to infer expression type",
                ErrorCode::TypeMissingReturn => "Non-unit function missing return value",
                ErrorCode::TypeInvalidBinaryOp => "Binary operator not defined for operand types",
                ErrorCode::TypeInvalidUnaryOp => "Unary operator not defined for operand type",
                ErrorCode::TypeInvalidMemberAccess => "Field or method does not exist on type",
                ErrorCode::TypeGenericMismatch => "Generic type argument mismatch",
                ErrorCode::TypeIncomparableOperands => {
                    "Ordering comparison over incompatible types: implicit conversions are forbidden"
                }
                ErrorCode::TypeUnknownMethod => {
                    "No method with this name exists for the receiver type"
                }
                ErrorCode::QuestionPropagation => {
                    "'?' requires the enclosing function to return a Result/Option-like type"
                }

                ErrorCode::BorrowUseAfterMove => "Use of moved value (use-after-move)",
                ErrorCode::BorrowCannotMutateImmutable => {
                    "Cannot mutate or reassign immutable binding"
                }
                ErrorCode::BorrowConflictActiveView => {
                    "Cannot mutate value while active immutable view exists"
                }
                ErrorCode::BorrowMultipleMutableViews => {
                    "Cannot create multiple concurrent mutable views"
                }
                ErrorCode::BorrowEscapingView => {
                    "Escaping view: reference cannot outlive local binding"
                }
                ErrorCode::BorrowConflict => {
                    "Borrow conflict: mutable reference exclusivity violation or reading during active &mut"
                }

                ErrorCode::EffectImpureInPureContext => "Side effect occurred in pure context",
                ErrorCode::EffectUnsafeOperation => {
                    "Unsafe operation without explicit unsafe block"
                }
                ErrorCode::EffectUnhandledIO => "Unhandled IO operation",

                ErrorCode::CodegenBackendFailed => "Native codegen backend failed",
                ErrorCode::CodegenLinkerFailed => "Linker execution failed",
                ErrorCode::CodegenIOError => "IO failure during artifact generation",

                ErrorCode::SecurityViolation => {
                    "Security Violation: Operation requires capability token"
                }
                ErrorCode::ProofCarryingCodeViolation => {
                    "Proof-Carrying Code Violation: Unproven operation"
                }
                ErrorCode::UncheckedFFIViolation => {
                    "Security Violation: Foreign call requires unsafe justification"
                }
                ErrorCode::DataRaceViolation => {
                    "Concurrency Violation: Potential data race across threads"
                }
                ErrorCode::AllocationViolation => {
                    "Real-Time Violation: Dynamic memory allocation in '@no_alloc' context"
                }
                ErrorCode::PanicViolation => {
                    "Real-Time Violation: Unproven panic path in '@no_panic' context"
                }
                ErrorCode::CImportHeaderNotFound => "C header file not found",
                ErrorCode::CImportReadFailed => "Failed to read C header file",
                ErrorCode::CImportUnsupportedConstruct => "Unsupported C construct in header file",
                ErrorCode::NonExhaustiveMatch => {
                    "Non-exhaustive patterns in match expression: missing patterns"
                }
                ErrorCode::UnreachablePattern => "Unreachable pattern in match expression",
                ErrorCode::LoopControlOutsideLoop => {
                    "'break' or 'continue' outside of a loop body (while/for/loop)"
                }
                ErrorCode::DimensionMismatch => {
                    "Dimension mismatch: incompatible units of measure in arithmetic expression"
                }
                ErrorCode::InvariantViolation => {
                    "Class invariant violation: class invariant cannot be proven upon method exit"
                }
                ErrorCode::TerminationViolation => {
                    "Termination violation: cannot prove function termination in pure context"
                }
                ErrorCode::RangeViolation => {
                    "Range violation: value exceeds static type interval or array bounds"
                }
                ErrorCode::ContractViolation => {
                    "Contract violation: precondition (requires) or postcondition (ensures) not satisfied"
                }
                ErrorCode::InternalVerification => {
                    "Internal DMIR intermediate representation verification failure"
                }
                ErrorCode::InlineAsmUnsupported => {
                    "Inline assembly is unsupported on the selected target backend"
                }
                ErrorCode::UnknownBinop => {
                    "Unknown binary operator encountered during code generation"
                }
                ErrorCode::CApiArgLimitExceeded => {
                    "C API argument limit exceeded (maximum 8 supported arguments)"
                }
                ErrorCode::AsyncBackendUnsupported => {
                    "Async execution (async/await) is unsupported on the selected target backend: requires PCS runtime"
                }
                ErrorCode::CrossCompilationMissingToolchain => {
                    "Cross-compilation toolchain not found for target triple (lld or cross-linker required)"
                }
                ErrorCode::InlineAlwaysExtern => {
                    "'@inline(always)' is rejected on extern declarations: the compiler cannot inline a body it does not have"
                }
                ErrorCode::InlineAttributeInvalid => {
                    "Malformed '@inline' attribute: expected '@inline', '@inline(always)' or '@inline(never)'"
                }
                ErrorCode::ArenaEscape => {
                    "A value allocated inside an @arena function escapes the arena region: only Copy types (Int, Float, Bool, Str) may be returned"
                }
                ErrorCode::AsmUnsupportedInst => {
                    "Instruction outside the supported asm safe subset: only mov/add/sub between GP registers, Int variables and integer immediates are accepted"
                }
                ErrorCode::AsmOperandNotInt => "Asm block operands must be Int variables",
                ErrorCode::AsmRequiresUnsafe => {
                    "Asm blocks must be wrapped in an unsafe(justification: \"...\") block"
                }
                ErrorCode::AsmUnsupportedBackend => {
                    "Structured asm blocks are not supported on this backend: use the Cranelift backend"
                }
                ErrorCode::PoolCapacityExceeded => {
                    "The @pool capacity is exceeded: the function provably allocates more values than the pool holds"
                }
                ErrorCode::OptimizationUnproven => {
                    "Optimizer layout transformation skipped: semantic equivalence could not be proven"
                }

                ErrorCode::DeprecatedFeature => {
                    "Deprecated language construct: use the modern equivalent"
                }
                ErrorCode::CanonicalKeyword => "Synonymous keyword: use the canonical spelling",
                ErrorCode::CanonicalTypeSpelling => {
                    "Synonymous type spelling: use the canonical type name"
                }
                ErrorCode::BoolIntComparison => {
                    "Comparison between Bool and an integer: operand types must match"
                }
                ErrorCode::BridgeTypeMismatch => "Bridge argument type mismatch",
                ErrorCode::BridgeUnsupportedType => {
                    "Unsupported type in declarative bridge function signature"
                }
                ErrorCode::BridgeUnknownLanguage => {
                    "Unknown foreign language in bridge declaration"
                }
                ErrorCode::CapMissingPermission => {
                    "Operation requires declared permission in datara.toml [capabilities]"
                }
                ErrorCode::CapGlobViolation => {
                    "Path or target address violates allowed glob in datara.toml [capabilities]"
                }
                ErrorCode::ClassKeywordForbidden => {
                    "The 'class' keyword does not exist in Datara: use 'struct' for data and 'behavior' or 'impl' for methods"
                }
                ErrorCode::StructMethodForbidden => {
                    "Methods cannot be defined inside 'struct': move methods into 'behavior <Type>' or 'impl <Trait> for <Type>'"
                }
                ErrorCode::ComponentMethodForbidden => {
                    "Components are pure POD data containers and cannot declare methods: move logic to a 'behavior'"
                }
                ErrorCode::ComponentNonPodField => {
                    "Component fields must be POD types (Int, Float, Bool, Char, or nested component): heap types are forbidden"
                }
                ErrorCode::BehaviorFieldForbidden => {
                    "Behaviors cannot declare state or fields: move fields into 'struct' or 'component'"
                }
                ErrorCode::BehaviorImpure => {
                    "Methods in a behavior are pure by default: impure operations (IO, Net) require explicit '#[effect(IO)]'"
                }
                ErrorCode::RoleCapabilityMissing => {
                    "Role capability requirement is not satisfied by the target type or environment"
                }
                ErrorCode::ImplCoherenceViolation => {
                    "Coherence violation: duplicate implementation of trait for this type"
                }
                ErrorCode::ImplMissingSuperTrait => {
                    "Missing super-trait implementation required by trait definition"
                }
                ErrorCode::UnprintableType => {
                    "Type is not printable: primitive type (Int, Float, Bool, Char, Str) or @derive(Display) is required"
                }
                ErrorCode::DeprecatedPrintFunction => {
                    "Print function is deprecated: use 'out' or 'err' statement instead"
                }
                ErrorCode::ComptimeRecursionLimit => {
                    "Recursion depth or step limit exceeded in comptime execution (E-CT-001)"
                }
                ErrorCode::ComptimeForbiddenEffect => {
                    "Effect not allowed in comptime execution: I/O, runtime builtins, and unsafe are forbidden (E-CT-002)"
                }
                ErrorCode::ComptimeNonConstArg => {
                    "Arguments to comptime function must be compile-time constants (E-CT-003)"
                }
                ErrorCode::MmioRequiresUnsafe => {
                    "MMIO access requires an 'unsafe' block or declaration in datara.toml [devices] (E-MMIO-001)"
                }
                ErrorCode::TargetRequiresLlvm => {
                    "Target is only supported via LLVM backend (E-TARGET-001)"
                }
            }
        }
    }
}
