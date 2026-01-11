---
description: Review code changes (PR, branch, commit, staged, unstaged, or audit)
agent: review
subtask: true
---

You are reviewing code for Audiobook Boss. Determine what to review based on the input provided.

---

Input: $ARGUMENTS

---

## Determining What to Review

Based on the input, determine which type of review to perform:

1. **No arguments (empty)**: Review all uncommitted changes
   - Run: `git diff` for unstaged changes
   - Run: `git diff --cached` for staged changes
   - Review both together

2. **"audit" keyword**: General quality audit of the current codebase
   - Focus on key directories: `src-tauri/src/`, `src/`
   - Review overall code health, patterns, and architecture
   - No diff required - read files directly

3. **PR number** (numeric only, e.g., "143"): Review that pull request
   - Run: `gh pr view $ARGUMENTS` to get PR context
   - Run: `gh pr diff $ARGUMENTS` to get the diff

4. **PR URL** (contains "github.com" or "pull"): Review that pull request
   - Extract the PR number from the URL
   - Run: `gh pr view <number>` and `gh pr diff <number>`

5. **Branch name** (exists in git): Compare current branch to that branch
   - Run: `git diff $ARGUMENTS...HEAD`
   - If comparing to main/master, shows what's new on current branch

6. **Commit hash** (7-40 hex characters): Review that specific commit
   - Run: `git show $ARGUMENTS`

Use your best judgment when the input is ambiguous. If unsure, ask for clarification.

---

## Gathering Context

**Diffs alone are not enough.** After getting the diff:
- Read the entire file(s) being modified to understand full context
- Code that looks wrong in isolation may be correct given surrounding logic
- Check for existing conventions in AGENTS.md, .editorconfig, etc.

---

## Review Process

1. Identify what changed (files, functions, logic)
2. Read full file context for each changed file
3. Run the required checks from the review agent guidelines
4. Analyze against project standards and engineering principles
5. Produce findings in the three-tier format

---

## Output Requirements

Follow the output format defined in the review agent:

- **🔴 Critical/High**: Behavioral bugs, security issues, must fix
- **🟡 Medium/Low**: Convention violations, architecture concerns
- **ℹ️ Informational**: Code hygiene observations (dead code, confusing patterns) - safe to defer

End with:
- Check results (fmt, clippy, test, contract, build)
- Engineering principle ratings (Design, Practice)
- Overall Code Quality rating (1-5)

Be direct, matter-of-fact, and helpful. No flattery.
