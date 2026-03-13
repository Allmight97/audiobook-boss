import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { access, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { constants as fsConstants } from 'node:fs';
import { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';

import { runIssue } from './runner';

const originalCwd = process.cwd();
const originalGhBin = process.env.GH_BIN;
const originalPath = process.env.PATH ?? '';

async function initializeGitRepo(repoRoot: string): Promise<void> {
	await writeFile(path.join(repoRoot, 'README.md'), '# temp repo\n', 'utf8');
	spawnSync('git', ['init', '-b', 'main'], { cwd: repoRoot, stdio: 'ignore' });
	spawnSync('git', ['init', '--bare', path.join(repoRoot, 'origin.git')], {
		cwd: repoRoot,
		stdio: 'ignore',
	});
	spawnSync('git', ['config', 'user.name', 'Test Runner'], { cwd: repoRoot, stdio: 'ignore' });
	spawnSync('git', ['config', 'user.email', 'test@example.com'], {
		cwd: repoRoot,
		stdio: 'ignore',
	});
	spawnSync('git', ['remote', 'add', 'origin', path.join(repoRoot, 'origin.git')], {
		cwd: repoRoot,
		stdio: 'ignore',
	});
	spawnSync('git', ['add', '.'], { cwd: repoRoot, stdio: 'ignore' });
	spawnSync('git', ['commit', '-m', 'init'], { cwd: repoRoot, stdio: 'ignore' });
	spawnSync('git', ['push', '-u', 'origin', 'main'], { cwd: repoRoot, stdio: 'ignore' });
}

async function createTestRepo(): Promise<{ repoRoot: string }> {
	const repoRoot = await mkdtemp(path.join(os.tmpdir(), 'abb-issue-runner-'));
	const binDir = path.join(repoRoot, 'bin');
	await mkdir(binDir, { recursive: true });

	await writeFile(
		path.join(binDir, 'codex'),
		[
			'#!/bin/sh',
			'out=""',
			'cwd="."',
			'while [ "$#" -gt 0 ]; do',
			'  if [ "$1" = "--output-last-message" ]; then',
			'    out="$2"',
			'    shift 2',
			'    continue',
			'  fi',
			'  if [ "$1" = "--cd" ]; then',
			'    cwd="$2"',
			'    shift 2',
			'    continue',
			'  fi',
			'  shift',
			'done',
			'prompt=$(cat)',
			'printf "%s\\n" "$prompt" > "$out"',
			'printf "\\nrunner change\\n" >> "$cwd/README.md"',
			'echo "fake codex run"',
		].join('\n'),
		'utf8',
	);
	await Bun.spawn(['chmod', '+x', path.join(binDir, 'codex')]).exited;

	await writeFile(
		path.join(binDir, 'gh'),
		[
			'#!/bin/sh',
			'cmd="$1 $2"',
			'case "$cmd" in',
			'  "issue view")',
			'    cat <<EOF',
			'{"number":42,"title":"Runner smoke","body":"<!-- abb:issue-kind=ready -->\\n\\n## Goal\\n\\nShip the runner.\\n\\n## Constraints\\n\\nKeep it local.\\n\\n## Acceptance\\n\\n- [ ] Opens a PR.\\n\\n## Validation\\n\\n- scripts/checks.sh standard\\n\\n## Delivery Mode\\n\\npr\\n\\n## Human Review\\n\\nvisual\\n\\n## Context\\n\\nIssue context.","url":"https://github.com/example/repo/issues/42","labels":[{"name":"enhancement"}]}',
			'EOF',
			'    ;;',
			'  "issue comment")',
			'    exit 0',
			'    ;;',
			'  "pr create")',
			'    echo "https://github.com/example/repo/pull/99"',
			'    ;;',
			'  "pr list")',
			'    echo "[]"',
			'    ;;',
			'  *)',
			'    echo "unsupported gh invocation: $*" >&2',
			'    exit 1',
			'    ;;',
			'esac',
		].join('\n'),
		'utf8',
	);
	await Bun.spawn(['chmod', '+x', path.join(binDir, 'gh')]).exited;

	await writeFile(path.join(binDir, 'bun'), ['#!/bin/sh', 'exit 0'].join('\n'), 'utf8');
	await Bun.spawn(['chmod', '+x', path.join(binDir, 'bun')]).exited;

	await writeFile(
		path.join(repoRoot, 'WORKFLOW.md'),
		[
			'---',
			'base_branch: main',
			'task_branch_prefix: issue/',
			'inbox_root: .agent-work/inbox',
			'worktree_root: .agent-work/worktrees',
			'run_root: .agent-work/runs',
			'lock_file: .agent-work/runner.lock.json',
			'codex_sandbox: workspace-write',
			'codex_approval: on-request',
			'---',
			'Task {{task.id}}',
			'Goal {{task.goal}}',
			'Constraints {{task.constraints}}',
			'Acceptance {{task.acceptance}}',
			'Context {{task.context}}',
			'Repo {{repo.root}}',
			'Base {{workflow.base_branch}}',
		].join('\n'),
		'utf8',
	);

	await mkdir(path.join(repoRoot, 'scripts'), { recursive: true });
	await writeFile(
		path.join(repoRoot, 'scripts/check-context-surface.sh'),
		'#!/bin/sh\nexit 0\n',
		'utf8',
	);
	await Bun.spawn(['chmod', '+x', path.join(repoRoot, 'scripts/check-context-surface.sh')]).exited;
	await writeFile(path.join(repoRoot, 'scripts/checks.sh'), '#!/bin/sh\nexit 0\n', 'utf8');
	await Bun.spawn(['chmod', '+x', path.join(repoRoot, 'scripts/checks.sh')]).exited;

	await writeFile(path.join(repoRoot, 'bunfig.toml'), '', 'utf8');
	await initializeGitRepo(repoRoot);
	return { repoRoot };
}

beforeEach(() => {
	process.chdir(originalCwd);
	process.env.PATH = originalPath;
	if (originalGhBin === undefined) {
		delete process.env.GH_BIN;
	} else {
		process.env.GH_BIN = originalGhBin;
	}
});

afterEach(() => {
	process.chdir(originalCwd);
	process.env.PATH = originalPath;
	if (originalGhBin === undefined) {
		delete process.env.GH_BIN;
	} else {
		process.env.GH_BIN = originalGhBin;
	}
});

describe('issues/runner', () => {
	test('runIssue creates a branch, commits changes, and opens a PR-backed run record', async () => {
		const { repoRoot } = await createTestRepo();
		process.chdir(repoRoot);
		process.env.PATH = `${path.join(repoRoot, 'bin')}:${originalPath}`;
		process.env.GH_BIN = path.join(repoRoot, 'bin', 'gh');

		try {
			const result = await runIssue(42);
			expect(result.status).toBe('pr-opened');
			expect(result.prUrl).toBe('https://github.com/example/repo/pull/99');
			expect(result.branchName).toBe('issue/issue-42');
			expect(result.validationCommands.length).toBeGreaterThan(0);

			const finalMessage = await readFile(result.finalMessagePath, 'utf8');
			expect(finalMessage).toContain('Task issue-42');
			await access(result.logPath, fsConstants.F_OK);
		} finally {
			process.chdir(originalCwd);
			process.env.PATH = originalPath;
			await rm(repoRoot, { recursive: true, force: true });
		}
	});
});
