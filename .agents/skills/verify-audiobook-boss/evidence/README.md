# Evidence

This directory is the named proof location for `verify-audiobook-boss`.

Cleanup tears down verification instances and scratch fixtures this run
created. It does not delete files here.

## What lands here

- Command transcripts (`*.transcript.txt`): argv, cwd, exit code, stdout, stderr
- Doctor reports (`*.doctor.json`)
- Dry-run plans (`*.dry-run.txt`): the launch/AX steps that were skipped
- Screenshots from a live Tauri window (`*.png`), when the GUI ran on macOS
- Pointers to `.logs/` session files for an instance this helper launched

Do not commit media fixtures. Synthesized audio lives in a temp scratch
directory recorded in the transcript, then deleted on cleanup.
