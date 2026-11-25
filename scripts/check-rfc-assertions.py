"""Run assertion-linked RFC cases and require every marked comparison to execute."""
from pathlib import Path
import runpy
import sys
import unittest


index = runpy.run_path(str(Path(__file__).with_name("rfc-test-index.py")))


def unpack(suite):
    for item in suite:
        if isinstance(item, unittest.TestSuite):
            yield from unpack(item)
        else:
            yield item


def run_witnesses(root, cases):
    strict = {key: case for key, case in cases.items() if case.assertions}
    if not strict:
        raise ValueError("no assertion-linked RFC cases")
    suite = unittest.TestSuite()
    locations = {}
    selected = set()
    for path in sorted({case.path for case in strict.values()}):
        file = root / path
        tests = unittest.TestLoader().discover(str(file.parent), pattern=file.name)
        for test in unpack(tests):
            name = test.id().split(".")[-1]
            key = f"{path}::{name}"
            if key not in strict:
                continue
            method = getattr(test, name)
            if Path(method.__code__.co_filename).resolve() != file.resolve() or key in selected:
                raise ValueError(f"{key}: ambiguous runtime test identity")
            selected.add(key)
            locations[method.__code__] = strict[key]
            suite.addTest(test)
    if selected != strict.keys():
        raise ValueError(f"strict tests absent from unittest discovery: {sorted(strict.keys() - selected)}")

    visited = set()

    def trace(frame, event, arg):
        if event == "call" and frame.f_code in locations:
            def visit(current, kind, _):
                if kind == "line":
                    visited.add((locations[current.f_code].key, current.f_lineno))
                return visit
            return visit
        return None

    previous = sys.gettrace()
    try:
        sys.settrace(trace)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
    finally:
        sys.settrace(previous)
    errors = []
    for key, case in strict.items():
        for id_, lines in case.assertions.items():
            errors.extend(f"{case.path}:{line} §{id_} {case.name}: assertion did not execute"
                          for line in lines if (key, line) not in visited)
    if errors:
        raise ValueError("\n".join(errors))
    if not result.wasSuccessful() or result.skipped or result.expectedFailures:
        raise ValueError("an assertion-linked test failed, was skipped or had an expected failure")
    total = sum(len(set(n for lines in case.assertions.values() for n in lines)) for case in strict.values())
    print(f"Verified {total} executed assertion lines in {len(strict)} RFC tests")


if __name__ == "__main__":
    try:
        root = index["ROOT"]
        cases = index["read_cases"](root)
        _, requirements = index["read_requirements"](root)
        index["reconcile"](cases, requirements)
        run_witnesses(root, cases)
    except ValueError as error:
        sys.exit(str(error))
