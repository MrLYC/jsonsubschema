# Benchmark: greenery vs Rust regex engine

Comparison of regex performance using the greenery Python library vs a Rust-based
DFA engine (regex-automata + PyO3).

**Environment:** Python 3.11, Linux x86_64

## Results

| Category | Scenario | Before (ms) | After (ms) | Speedup |
|----------|----------|-------------|------------|---------|
| Primitive | null <: null | 0.477 | 0.476 | 1.0x |
| Primitive | boolean <: boolean | 0.479 | 0.453 | 1.1x |
| Primitive | integer <: number | 0.532 | 0.497 | 1.1x |
| Primitive | integer NOT <: string | 0.626 | 0.498 | 1.3x |
| Primitive | type list equivalence | 1.972 | 1.951 | 1.0x |
| String | minLength subtype | 0.864 | 0.885 | 1.0x |
| **String** | **pattern subset (simple)** | **11.966** | **0.729** | **16.4x** |
| **String** | **pattern subset (complex)** | **41.091** | **0.719** | **57.2x** |
| **String** | **pattern + length combo** | **151.054** | **1.755** | **86.1x** |
| String | enum string subset | 4.110 | 3.996 | 1.0x |
| Numeric | integer range subtype | 0.642 | 0.609 | 1.1x |
| Numeric | multipleOf subtype | 0.591 | 0.575 | 1.0x |
| Numeric | float multipleOf | 0.634 | 0.635 | 1.0x |
| Numeric | number exclusiveMin/Max | 0.691 | 0.655 | 1.1x |
| Enum | simple enum subset | 3.881 | 3.844 | 1.0x |
| Enum | heterogeneous enum | 5.666 | 5.602 | 1.0x |
| Enum | array enum | 7.070 | 6.959 | 1.0x |
| Enum | object enum | 5.781 | 5.609 | 1.0x |
| Enum | large string enum (20) | 19.355 | 19.218 | 1.0x |
| Array | simple items subtype | 1.345 | 1.326 | 1.0x |
| Array | tuple items | 2.633 | 2.634 | 1.0x |
| Array | minItems/maxItems range | 1.196 | 1.146 | 1.0x |
| Array | nested array (150x4) | 3.120 | 3.108 | 1.0x |
| Array | contains (Draft-07) | 0.872 | 0.841 | 1.0x |
| Object | simple properties | 2.080 | 2.082 | 1.0x |
| Object | additionalProperties false | 1.448 | 1.430 | 1.0x |
| **Object** | **patternProperties** | **13.004** | **1.923** | **6.8x** |
| Object | deeply nested (4 levels) | 5.887 | 5.649 | 1.0x |
| Object | 10 properties mixed | 8.661 | 8.320 | 1.0x |
| Connective | anyOf subtype | 3.526 | 3.371 | 1.0x |
| Connective | allOf simplification | 1.228 | 1.163 | 1.1x |
| Connective | oneOf expansion | 8.284 | 8.057 | 1.0x |
| Connective | not (string) | 1.502 | 1.487 | 1.0x |
| Connective | not (with minLength) | 1.861 | 1.831 | 1.0x |
| Negation | not int multipleOf (bounded) | 11.204 | 10.871 | 1.0x |
| Negation | not num multipleOf (bounded) | 8.390 | 7.712 | 1.1x |
| Negation | not array minItems | 2.164 | 2.017 | 1.1x |
| Negation | not object maxProperties | 2.254 | 2.083 | 1.1x |
| Draft-07 | if/then/else basic | 8.283 | 7.505 | 1.1x |
| Draft-07 | if/then (no else) | 8.661 | 8.601 | 1.0x |
| Draft-07 | contains subtype | 0.879 | 0.880 | 1.0x |
| SchemaDiff | equivalent schemas | 0.992 | 1.000 | 1.0x |
| SchemaDiff | backward compatible | 10.096 | 9.693 | 1.0x |
| SchemaDiff | breaking change | 0.992 | 0.966 | 1.0x |
| SchemaDiff | Washington Post example | 12.754 | 12.083 | 1.1x |
| Real-World | ML operator self-check | 21.219 | 20.926 | 1.0x |
| Real-World | ML operator v1 vs v2 | 21.592 | 21.213 | 1.0x |
| Real-World | ML operator diff | 43.823 | 42.947 | 1.0x |
| Real-World | Iris data <: input | 7.472 | 7.264 | 1.0x |
| Real-World | Input NOT <: Iris | 7.221 | 7.118 | 1.0x |
| Canonicalize | simple type | 0.264 | 0.258 | 1.0x |
| Canonicalize | type list | 1.180 | 1.152 | 1.0x |
| Canonicalize | oneOf expansion | 16.299 | 16.188 | 1.0x |
| Canonicalize | ML operator schema | 8.313 | 8.099 | 1.0x |
| Canonicalize | if/then/else | 7.088 | 7.088 | 1.0x |
| Stress | 50-property object | 37.410 | 37.462 | 1.0x |
| Stress | 8-level nested object | 27.594 | 27.186 | 1.0x |
| Stress | 20-branch anyOf | 7.986 | 7.893 | 1.0x |
| Stress | 100-value string enum | 96.158 | 95.715 | 1.0x |
| **TOTAL** | | **684.417** | **463.953** | **1.5x** |

## Regex-specific speedups

| Scenario | Before (ms) | After (ms) | Speedup |
|----------|-------------|------------|---------|
| pattern subset (simple) | 11.966 | 0.729 | **16.4x** |
| pattern subset (complex) | 41.091 | 0.719 | **57.2x** |
| pattern + length combo | 151.054 | 1.755 | **86.1x** |
| patternProperties | 13.004 | 1.923 | **6.8x** |

Regex-heavy operations see **16x-86x** speedups. Non-regex scenarios are unaffected
since the Rust engine only replaces the regex subsystem.
