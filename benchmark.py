#!/usr/bin/env python3
"""Benchmark for jsonsubschema — outputs JSON for comparison."""

import json
import statistics
import sys
import time


def bench(fn, *args, repeat=20, warmup=2):
    for _ in range(warmup):
        fn(*args)
    times = []
    for _ in range(repeat):
        t0 = time.perf_counter()
        result = fn(*args)
        t1 = time.perf_counter()
        times.append((t1 - t0) * 1000)
    return result, statistics.median(times), min(times), max(times)


def run_benchmarks():
    from jsonsubschema import isSubschema, schemaDiff
    from jsonsubschema._canonicalization import (
        canonicalize_schema, simplify_schema_and_embed_checkers,
    )

    results = []

    def record(category, name, fn, *args, **kwargs):
        res, med, mn, mx = bench(fn, *args, **kwargs)
        results.append({
            "category": category, "name": name,
            "result": str(res)[:20],
            "median_ms": round(med, 3),
            "min_ms": round(mn, 3), "max_ms": round(mx, 3),
        })

    # 1. Primitive
    record("Primitive", "null <: null", isSubschema, {"type": "null"}, {"type": "null"})
    record("Primitive", "boolean <: boolean", isSubschema, {"type": "boolean"}, {"type": "boolean"})
    record("Primitive", "integer <: number", isSubschema, {"type": "integer"}, {"type": "number"})
    record("Primitive", "integer NOT <: string", isSubschema, {"type": "integer"}, {"type": "string"})
    record("Primitive", "type list equivalence", isSubschema, {"type": ["null", "string"]}, {"type": ["string", "null"]})

    # 2. String + Regex
    record("String", "minLength subtype", isSubschema,
           {"type": "string", "minLength": 5}, {"type": "string", "minLength": 3})
    record("String", "pattern subset (simple)", isSubschema,
           {"type": "string", "pattern": "^[a-z]+$"}, {"type": "string", "pattern": "^[a-z]*$"})
    record("String", "pattern subset (complex)", isSubschema,
           {"type": "string", "pattern": "^(ab)+$"}, {"type": "string", "pattern": "^(ab)*$"})
    record("String", "pattern + length combo", isSubschema,
           {"type": "string", "pattern": "^[A-Z]{3}$"}, {"type": "string", "maxLength": 5})
    record("String", "enum string subset", isSubschema,
           {"type": "string", "enum": ["a", "b"]}, {"type": "string", "enum": ["a", "b", "c"]})

    # 3. Numeric
    record("Numeric", "integer range subtype", isSubschema,
           {"type": "integer", "minimum": 5, "maximum": 10}, {"type": "integer", "minimum": 0, "maximum": 20})
    record("Numeric", "multipleOf subtype", isSubschema,
           {"type": "integer", "multipleOf": 6}, {"type": "integer", "multipleOf": 3})
    record("Numeric", "float multipleOf", isSubschema,
           {"type": "number", "multipleOf": 0.5, "minimum": 0, "maximum": 10},
           {"type": "number", "multipleOf": 0.25})
    record("Numeric", "number exclusiveMin/Max", isSubschema,
           {"type": "number", "minimum": 1, "maximum": 9, "exclusiveMinimum": True, "exclusiveMaximum": True},
           {"type": "number", "minimum": 0, "maximum": 10})

    # 4. Enum
    record("Enum", "simple enum subset", isSubschema, {"enum": [1, 2]}, {"enum": [1, 2, 3]})
    record("Enum", "heterogeneous enum", isSubschema, {"enum": [1, "a"]}, {"enum": [1, "a", True]})
    record("Enum", "array enum", isSubschema, {"type": "array", "enum": [[1, 2], [3]]}, {"type": "array"})
    record("Enum", "object enum", isSubschema, {"type": "object", "enum": [{"a": 1}, {"b": "x"}]}, {"type": "object"})
    record("Enum", "large string enum (20)", isSubschema,
           {"type": "string", "enum": [f"val_{i}" for i in range(20)]}, {"type": "string"})

    # 5. Array
    record("Array", "simple items subtype", isSubschema,
           {"type": "array", "items": {"type": "integer"}}, {"type": "array", "items": {"type": "number"}})
    record("Array", "tuple items", isSubschema,
           {"type": "array", "items": [{"type": "string"}, {"type": "integer"}], "additionalItems": False},
           {"type": "array", "items": [{"type": "string"}, {"type": "number"}], "additionalItems": False})
    record("Array", "minItems/maxItems range", isSubschema,
           {"type": "array", "minItems": 2, "maxItems": 5}, {"type": "array", "minItems": 1, "maxItems": 10})
    record("Array", "nested array (150x4)", isSubschema,
           {"type": "array", "items": {"type": "array", "items": {"type": "number"}, "minItems": 4, "maxItems": 4}, "minItems": 150, "maxItems": 150},
           {"type": "array", "items": {"type": "array", "items": {"type": "number"}}})
    record("Array", "contains (Draft-07)", isSubschema,
           {"type": "array", "contains": {"type": "integer"}}, {"type": "array", "contains": {"type": "number"}})

    # 6. Object
    record("Object", "simple properties", isSubschema,
           {"type": "object", "properties": {"name": {"type": "string"}, "age": {"type": "integer"}}, "required": ["name", "age"]},
           {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]})
    record("Object", "additionalProperties false", isSubschema,
           {"type": "object", "properties": {"x": {"type": "integer"}}, "additionalProperties": False},
           {"type": "object", "properties": {"x": {"type": "number"}}})
    record("Object", "patternProperties", isSubschema,
           {"type": "object", "patternProperties": {"^s_": {"type": "string"}}, "additionalProperties": False},
           {"type": "object", "patternProperties": {"^s_": {"type": "string"}}})
    record("Object", "deeply nested (4 levels)", isSubschema,
           {"type": "object", "properties": {"l1": {"type": "object", "properties": {"l2": {"type": "object", "properties": {"l3": {"type": "object", "properties": {"val": {"type": "integer"}}, "required": ["val"]}}, "required": ["l3"]}}, "required": ["l2"]}}, "required": ["l1"]},
           {"type": "object", "properties": {"l1": {"type": "object", "properties": {"l2": {"type": "object", "properties": {"l3": {"type": "object"}}}}}}})
    record("Object", "10 properties mixed", isSubschema,
           {"type": "object", "properties": {f"prop{i}": {"type": "integer", "minimum": 0, "maximum": 100} for i in range(10)}, "additionalProperties": False},
           {"type": "object", "properties": {f"prop{i}": {"type": "number"} for i in range(10)}})

    # 7. Connectives
    record("Connective", "anyOf subtype", isSubschema,
           {"anyOf": [{"type": "string"}, {"type": "integer"}]}, {"anyOf": [{"type": "string"}, {"type": "number"}]})
    record("Connective", "allOf simplification", isSubschema,
           {"allOf": [{"type": "number", "minimum": 0}, {"type": "number", "maximum": 100}]},
           {"type": "number", "minimum": 0, "maximum": 100})
    record("Connective", "oneOf expansion", isSubschema,
           {"oneOf": [{"type": "string"}, {"type": "integer"}]}, {"anyOf": [{"type": "string"}, {"type": "number"}]})
    record("Connective", "not (string)", isSubschema, {"type": "integer"}, {"not": {"type": "string"}})
    record("Connective", "not (with minLength)", isSubschema, {"type": "integer"}, {"not": {"type": "string", "minLength": 1}})

    # 8. Negation
    record("Negation", "not int multipleOf (bounded)", isSubschema,
           {"type": "integer", "minimum": 1, "maximum": 2},
           {"not": {"type": "integer", "minimum": 0, "maximum": 10, "multipleOf": 3}})
    record("Negation", "not num multipleOf (bounded)", isSubschema,
           {"type": "number", "minimum": 1, "maximum": 4},
           {"not": {"type": "number", "minimum": 0, "maximum": 10, "multipleOf": 5}})
    record("Negation", "not array minItems", isSubschema,
           {"type": "array", "maxItems": 2}, {"not": {"type": "array", "minItems": 3}})
    record("Negation", "not object maxProperties", isSubschema,
           {"type": "object", "minProperties": 6}, {"not": {"type": "object", "maxProperties": 5}})

    # 9. Draft-07
    record("Draft-07", "if/then/else basic", isSubschema,
           {"type": "string", "minLength": 1},
           {"if": {"type": "string"}, "then": {"minLength": 1}, "else": {"type": "integer"}})
    record("Draft-07", "if/then (no else)", isSubschema,
           {"type": "integer"}, {"if": {"type": "string"}, "then": {"minLength": 1}})
    # Skipped: if/then/else + type triggers a portion library edge case
    # record("Draft-07", "if/then/else + type", isSubschema,
    #        {"type": "integer", "minimum": 0, "maximum": 100},
    #        {"type": "integer", "if": {"minimum": 0}, "then": {"maximum": 100}, "else": {"minimum": -100}})
    record("Draft-07", "contains subtype", isSubschema,
           {"type": "array", "contains": {"type": "integer"}}, {"type": "array", "contains": {"type": "number"}})

    # 10. SchemaDiff
    record("SchemaDiff", "equivalent schemas", schemaDiff, {"type": "string"}, {"type": "string"})
    record("SchemaDiff", "backward compatible", schemaDiff,
           {"type": "object", "properties": {"cat": {"type": "string", "enum": ["a", "b"]}}},
           {"type": "object", "properties": {"cat": {"type": "string", "enum": ["a", "b", "c"]}}})
    record("SchemaDiff", "breaking change", schemaDiff, {"type": "string"}, {"type": "integer"})
    record("SchemaDiff", "Washington Post example", schemaDiff,
           {"type": "object", "properties": {"category": {"type": "string", "enum": ["staff", "wires", "other"]}}},
           {"type": "object", "properties": {"category": {"type": "string", "enum": ["staff", "wires", "stock", "other"]}}})

    # 11. Real-World
    ml_op = {"type": "object", "properties": {
        "loss": {"enum": ["deviance", "exponential"]}, "lr": {"type": "number", "minimum": 0.01, "maximum": 1.0},
        "n_est": {"type": "integer", "minimum": 10, "maximum": 100}, "sub": {"type": "number", "minimum": 0.01, "maximum": 1.0},
        "split": {"type": "number", "minimum": 0.01, "maximum": 0.5}, "leaf": {"type": "number", "minimum": 0.01, "maximum": 0.5},
        "depth": {"type": "integer", "minimum": 3, "maximum": 5}, "feat": {"enum": ["auto", "sqrt", "log2", None]},
        "presort": {"enum": ["auto"]}, "tol": {"type": "number", "minimum": 1e-08, "maximum": 0.01}},
        "additionalProperties": False, "required": ["presort"]}
    ml_v2 = json.loads(json.dumps(ml_op))
    ml_v2["properties"]["n_iter"] = {"type": "integer", "minimum": 5, "maximum": 10}

    record("Real-World", "ML operator self-check", isSubschema, ml_op, ml_op)
    record("Real-World", "ML operator v1 vs v2", isSubschema, ml_op, ml_v2)
    record("Real-World", "ML operator diff", schemaDiff, ml_op, ml_v2)

    iris_data = {"type": "object", "properties": {
        "X": {"type": "array", "items": {"type": "array", "items": {"type": "number"}, "minItems": 4, "maxItems": 4}, "minItems": 150, "maxItems": 150},
        "y": {"type": "array", "items": {"type": "integer"}, "minItems": 150, "maxItems": 150}}, "required": ["X", "y"], "additionalProperties": False}
    iris_input = {"type": "object", "properties": {
        "X": {"type": "array", "items": {"type": "array", "items": {"type": "number"}}},
        "y": {"type": "array", "items": {"type": "number"}}}, "required": ["X", "y"]}
    record("Real-World", "Iris data <: input", isSubschema, iris_data, iris_input)
    record("Real-World", "Input NOT <: Iris", isSubschema, iris_input, iris_data)

    # 12. Canonicalize
    def canonicalize_only(s):
        return simplify_schema_and_embed_checkers(canonicalize_schema(s))
    record("Canonicalize", "simple type", canonicalize_only, {"type": "integer", "minimum": 0})
    record("Canonicalize", "type list", canonicalize_only, {"type": ["string", "null", "integer"]})
    record("Canonicalize", "oneOf expansion", canonicalize_only,
           {"oneOf": [{"type": "string"}, {"type": "integer"}, {"type": "null"}]})
    record("Canonicalize", "ML operator schema", canonicalize_only, ml_op)
    record("Canonicalize", "if/then/else", canonicalize_only,
           {"if": {"type": "string"}, "then": {"minLength": 1}, "else": {"type": "integer"}})

    # 13. Stress
    record("Stress", "50-property object", isSubschema,
           {"type": "object", "properties": {f"p{i}": {"type": "integer", "minimum": 0} for i in range(50)}, "additionalProperties": False},
           {"type": "object", "properties": {f"p{i}": {"type": "number"} for i in range(50)}})

    def make_nested(depth, leaf):
        if depth == 0: return leaf
        return {"type": "object", "properties": {"child": make_nested(depth - 1, leaf)}, "required": ["child"]}
    record("Stress", "8-level nested object", isSubschema,
           make_nested(8, {"type": "integer", "minimum": 0}), make_nested(8, {"type": "number"}))
    record("Stress", "20-branch anyOf", isSubschema,
           {"anyOf": [{"type": "integer", "minimum": i*10, "maximum": i*10+9} for i in range(20)]},
           {"type": "integer", "minimum": 0, "maximum": 199})
    record("Stress", "100-value string enum", isSubschema,
           {"type": "string", "enum": [f"option_{i}" for i in range(100)]}, {"type": "string"})

    return results


def print_table(results):
    categories = {}
    for r in results:
        categories.setdefault(r["category"], []).append(r)
    max_name = max(len(r["name"]) for r in results)
    sep = "-" * (max_name + 52)
    print(f"\n{'Scenario':<{max_name}}  {'Result':>8}  {'Median':>9}  {'Min':>9}  {'Max':>9}")
    print(sep)
    total = 0
    for cat, items in categories.items():
        print(f"\n[{cat}]")
        for r in items:
            print(f"  {r['name']:<{max_name}}  {r['result'][:8]:>8}  {r['median_ms']:>8.3f}  {r['min_ms']:>8.3f}  {r['max_ms']:>8.3f}  ms")
            total += r["median_ms"]
    print(sep)
    print(f"  TOTAL: {total:.3f} ms  ({len(results)} scenarios)")


if __name__ == "__main__":
    results = run_benchmarks()
    if "--json" in sys.argv:
        out = sys.argv[sys.argv.index("--json") + 1] if len(sys.argv) > sys.argv.index("--json") + 1 else "/dev/stdout"
        with open(out, "w") as f:
            json.dump(results, f, indent=2)
        print(f"Saved {len(results)} results to {out}", file=sys.stderr)
    else:
        print_table(results)
