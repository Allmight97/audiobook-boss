from __future__ import annotations

from stop_repo_guard import main


# Transitional entrypoint for Codex sessions that loaded the old Stop hook list
# before the 1.0.20 hook consolidation. Remove once active sessions reliably
# execute stop_repo_guard.py from .codex/hooks.json.
if __name__ == "__main__":
    main()
