#!/usr/bin/env python3
"""Select listed Files tests using case-local substring or exact matching."""
import argparse
import sys


def select(lines, cases, pattern, *, exact=False, all_cases=False):
    for line in lines:
        if not line.rstrip().endswith(": test"):
            continue
        name = line.rstrip().removesuffix(": test")
        if not name.startswith("cases::"):
            continue
        case, _, relative = name.removeprefix("cases::").partition("::")
        if not all_cases and case not in cases:
            continue
        candidate = name if all_cases else relative
        if not pattern or (candidate == pattern if exact else pattern in candidate):
            yield name


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--filter", default="")
    parser.add_argument("--exact", action="store_true")
    parser.add_argument("--all-cases", action="store_true")
    parser.add_argument("cases", nargs="*")
    args = parser.parse_args()
    for name in select(sys.stdin, set(args.cases), args.filter,
                       exact=args.exact, all_cases=args.all_cases):
        print(name)


if __name__ == "__main__":
    main()
