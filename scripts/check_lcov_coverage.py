from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

HUNK_PATTERN = re.compile(r"@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@")
PRODUCTION_PATH_PREFIXES = ("crates/",)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Enforce total and incremental line coverage from LCOV.")
    parser.add_argument("--lcov", required=True)
    parser.add_argument("--threshold", type=float, required=True)
    parser.add_argument("--base-sha", default="")
    parser.add_argument("--head-sha", default="")
    return parser.parse_args()


def load_lcov(path: Path) -> dict[str, dict[int, bool]]:
    repo_root = Path.cwd().resolve()
    coverage: dict[str, dict[int, bool]] = {}
    current_file: str | None = None

    for raw_line in path.read_text(encoding="utf-8").splitlines():
        if raw_line.startswith("SF:"):
            source = Path(raw_line[3:]).resolve()
            try:
                relative = source.relative_to(repo_root).as_posix()
            except ValueError:
                current_file = None
                continue
            current_file = relative
            coverage.setdefault(relative, {})
            continue
        if raw_line.startswith("DA:") and current_file is not None:
            number_text, hits_text = raw_line[3:].split(",", 1)
            coverage[current_file][int(number_text)] = int(hits_text) > 0
            continue
        if raw_line == "end_of_record":
            current_file = None

    return {
        path: lines
        for path, lines in coverage.items()
        if is_production_rust_path(path) and lines
    }


def total_coverage(coverage: dict[str, dict[int, bool]]) -> tuple[int, int]:
    executable = sum(len(lines) for lines in coverage.values())
    covered = sum(1 for lines in coverage.values() for covered in lines.values() if covered)
    return covered, executable


def load_changed_lines(base_sha: str, head_sha: str) -> dict[str, set[int]]:
    command = ["git", "diff", "--unified=0", base_sha]
    if head_sha != "WORKTREE":
        command.append(head_sha)
    command.extend(["--", "crates"])
    result = subprocess.run(command, check=True, capture_output=True, text=True)
    changed: dict[str, set[int]] = {}
    current_file: str | None = None
    new_line_number = 0

    for raw_line in result.stdout.splitlines():
        if raw_line.startswith("+++ b/"):
            candidate = raw_line[6:]
            current_file = candidate if is_production_rust_path(candidate) else None
            if current_file is not None:
                changed.setdefault(current_file, set())
            continue
        if raw_line.startswith("+++ "):
            current_file = None
            continue
        if raw_line.startswith("@@"):
            match = HUNK_PATTERN.match(raw_line)
            if match is None:
                raise ValueError(f"unsupported diff hunk header: {raw_line}")
            new_line_number = int(match.group(1))
            continue
        if current_file is None:
            continue
        if raw_line.startswith("+") and not raw_line.startswith("+++"):
            changed[current_file].add(new_line_number)
            new_line_number += 1
            continue
        if raw_line.startswith("-") and not raw_line.startswith("---"):
            continue
        new_line_number += 1

    return changed


def is_uncomparable_git_range(error: subprocess.CalledProcessError) -> bool:
    stderr = (error.stderr or "").lower()
    return error.returncode == 128 and (
        "bad object" in stderr
        or "unknown revision" in stderr
        or "invalid revision range" in stderr
    )


def is_production_rust_path(path: str) -> bool:
    return (
        path.endswith(".rs")
        and path.startswith(PRODUCTION_PATH_PREFIXES)
        and "/tests/" not in path
        and "/src/bin/" not in path
    )


def ratio(covered: int, executable: int) -> float:
    if executable == 0:
        return 100.0
    return covered / executable * 100.0


def main() -> int:
    args = parse_args()
    coverage = load_lcov(Path(args.lcov))
    covered, executable = total_coverage(coverage)
    total = ratio(covered, executable)
    print(f"Total line coverage: {covered}/{executable} executable lines covered ({total:.2f}%).")
    if total + 1e-9 < args.threshold:
        print(f"Required total coverage threshold: {args.threshold:.2f}%.")
        return 1

    if not args.base_sha or not args.head_sha or args.base_sha == "0000000000000000000000000000000000000000":
        print("No comparable git range was provided; skipping incremental coverage gate.")
        return 0

    try:
        changed_lines = load_changed_lines(args.base_sha, args.head_sha)
    except subprocess.CalledProcessError as error:
        if is_uncomparable_git_range(error):
            print("No comparable git range was available locally; skipping incremental coverage gate.")
            return 0
        raise

    changed_executable = 0
    changed_covered = 0
    uncovered: list[str] = []
    for relative_path, lines in changed_lines.items():
        file_coverage = coverage.get(relative_path, {})
        executable_lines = sorted(line for line in lines if line in file_coverage)
        changed_executable += len(executable_lines)
        for line in executable_lines:
            if file_coverage[line]:
                changed_covered += 1
            else:
                uncovered.append(f"{relative_path}:{line}")

    if changed_executable == 0:
        print("No changed executable Rust lines were found under crates/; skipping incremental coverage gate.")
        return 0

    incremental = ratio(changed_covered, changed_executable)
    print(
        "Incremental line coverage: "
        f"{changed_covered}/{changed_executable} changed executable lines covered ({incremental:.2f}%)."
    )
    if incremental + 1e-9 < args.threshold:
        print(f"Required incremental coverage threshold: {args.threshold:.2f}%.")
        if uncovered:
            print("Uncovered changed executable lines:")
            for entry in uncovered:
                print(f"- {entry}")
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
