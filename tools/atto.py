#!/usr/bin/env python3
"""Source-free Atto toolchain inventory. Detection does not prove compiler execution."""

import argparse
import json
import shutil
from typing import Callable

BACKENDS = {
    "rust": ("cargo", "rustc"),
    "java": ("java", "javac"),
    "llvm": ("clang", "cmake"),
    "wasm": ("node",),
    "net": ("dotnet",),
    "forge": ("c++", "cmake"),
}


def inventory(which: Callable[[str], str | None] = shutil.which) -> dict:
    result = {}
    for backend, tools in sorted(BACKENDS.items()):
        detected = {tool: bool(which(tool)) for tool in tools}
        result[backend] = {
            "tools_found": detected,
            "all_tools_found": all(detected.values()),
        }
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="AttoLang bootstrap utilities (preview)")
    subcommands = parser.add_subparsers(dest="command", required=True)
    doctor = subcommands.add_parser("doctor", help="check tool availability without executing binaries")
    doctor.add_argument("--json", action="store_true", help="emit versioned JSON")
    doctor.add_argument("--require", choices=sorted(BACKENDS), action="append", default=[],
                        help="fail if this backend's prerequisite tools are not found (repeatable)")
    args = parser.parse_args(argv)
    if args.command != "doctor":
        parser.error("unsupported command")
    detected = inventory()
    missing = sorted({backend for backend in args.require
                      if not detected[backend]["all_tools_found"]})
    report = {
        "schema_version": 1,
        "evidence": "PATH lookup only; no compiler/build or guest execution performed",
        "backends": detected,
        "missing_required": missing,
    }
    if args.json:
        print(json.dumps(report, sort_keys=True, indent=2))
    else:
        for backend, status in sorted(detected.items()):
            found = ", ".join(f"{name}={'found' if ok else 'missing'}"
                              for name, ok in sorted(status["tools_found"].items()))
            print(f"{backend}: {found}")
        if missing:
            print("Missing requested toolchains: " + ", ".join(missing))
        print(report["evidence"])
    return 2 if missing else 0


if __name__ == "__main__":
    raise SystemExit(main())
