from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


EXPECTED_CONTRACT_VERSION = 8


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def field_shape(type_contract: dict[str, Any]) -> list[tuple[str, str, bool]]:
    return [
        (field["name"], field["type"], field.get("required", False))
        for field in type_contract["fields"]
    ]


def check_contract(contract_path: Path) -> None:
    contract = json.loads(contract_path.read_text(encoding="utf-8"))
    require(
        contract.get("contractVersion") == EXPECTED_CONTRACT_VERSION,
        f"expected SDK contract version {EXPECTED_CONTRACT_VERSION}",
    )

    types = contract["types"]
    lifecycle = types["OperationLifecycleEvent"]
    require(
        field_shape(lifecycle)
        == [("operation_id", "u64", True), ("state", "OperationState", True)],
        "OperationLifecycleEvent field contract drifted",
    )
    require(
        lifecycle.get("terminalMapping")
        == {
            "completed": "success",
            "cancelled": "cancelled",
            "superseded": "dropped",
            "failed": "error",
        },
        "OperationLifecycleEvent terminal mapping drifted",
    )

    terminal = types["TerminalEvent"]
    require(
        terminal.get("representation") == "tagged-union",
        "TerminalEvent is no longer a tagged union",
    )
    require(
        terminal.get("variants") == ["runtime", "lifecycle"],
        "TerminalEvent variants drifted",
    )
    require(
        terminal.get("variantTypes")
        == {"runtime": "RuntimeEvent", "lifecycle": "OperationLifecycleEvent"},
        "TerminalEvent variant types drifted",
    )

    result = types["NnrpResult"]
    require(
        field_shape(result)
        == [
            ("operation_id", "u64", True),
            ("terminal_state", "ResultTerminalState", True),
            ("event", "TerminalEvent", True),
        ],
        "NnrpResult field contract drifted",
    )

    rust = contract["languageProjections"]["rust"]
    require(
        rust.get("operationLifecycleEvent") == "nnrp_runtime::OperationLifecycleEvent",
        "Rust OperationLifecycleEvent projection drifted",
    )
    require(
        rust.get("terminalEvent") == "nnrp_runtime::NnrpTerminalEvent",
        "Rust NnrpTerminalEvent projection drifted",
    )
    require(
        rust.get("result") == "nnrp_runtime::NnrpResult",
        "Rust NnrpResult projection drifted",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--contract", type=Path, required=True)
    args = parser.parse_args()
    check_contract(args.contract)


if __name__ == "__main__":
    main()
