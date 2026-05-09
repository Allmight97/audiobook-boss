from __future__ import annotations

from common import (
    changed_paths,
    docs_only_paths,
    docs_surface_touched,
    emit,
    fallback_surface_touched,
    ipc_surface_touched,
    meaningful_changed_paths,
    run_command,
    ui_surface_touched,
)


def check_context_surface(messages: list[str], failures: list[str]) -> None:
    result = run_command(["bash", "scripts/check-context-surface.sh"])
    if result.returncode == 0:
        messages.append("docs/skill/hook surface OK")
        return
    detail = (result.stderr or result.stdout).strip() or "check-context-surface failed"
    failures.append("ABB docs/skill surface drift detected.\n" + detail)


def check_ipc_bindings(messages: list[str], failures: list[str]) -> None:
    result = run_command(["bash", "scripts/check-generated-bindings.sh", "--mode", "local"])
    if result.returncode == 0:
        messages.append("IPC generated bindings OK")
        return
    detail = "\n".join(part.strip() for part in (result.stdout, result.stderr) if part.strip())
    failures.append(
        "ABB IPC generated bindings drift detected.\n"
        + (detail or "Run `bun run bindings:generate` before finishing.")
    )


def check_fallback_policy(messages: list[str], failures: list[str]) -> None:
    result = run_command(["bash", "scripts/check-fallback-policy.sh"])
    if result.returncode == 0:
        messages.append("fallback policy OK")
        return
    detail = "\n".join(part.strip() for part in (result.stdout, result.stderr) if part.strip())
    failures.append(
        "ABB fallback policy violation detected.\n"
        + (detail or "Run `bash scripts/check-fallback-policy.sh` and fix the reported fallback metadata.")
    )


def verification_message(paths: list[str]) -> str:
    if docs_only_paths(paths):
        return "verification lane: docs-only -> run `bash scripts/check-context-surface.sh`"
    return "verification lane: code/config/build -> run `scripts/checks.sh standard`"


def main() -> None:
    paths = meaningful_changed_paths(changed_paths())
    if not paths:
        emit({"continue": True})
        return

    messages: list[str] = []
    failures: list[str] = []

    if docs_surface_touched(paths):
        check_context_surface(messages, failures)
    if ipc_surface_touched(paths):
        check_ipc_bindings(messages, failures)
    if fallback_surface_touched(paths):
        check_fallback_policy(messages, failures)

    messages.append(verification_message(paths))
    if ui_surface_touched(paths):
        messages.append(
            "UI-adjacent change: pair targeted tests with browser-agent or human visual/UX review"
        )

    if failures:
        emit(
            {
                "decision": "block",
                "reason": "\n\n".join(failures),
                "systemMessage": "\n".join(failures),
            }
        )
        return

    emit({"continue": True, "systemMessage": "ABB repo guard: " + "; ".join(messages)})


if __name__ == "__main__":
    main()
