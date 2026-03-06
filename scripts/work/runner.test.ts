import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { access, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { constants as fsConstants } from 'node:fs';
import { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';

import { loadWorkflowConfig } from './common';
import { ensureTaskWorkspace, getTaskBranchName } from './git';
import { finishTask, gcRunnerState, runTaskById } from './runner';

const originalCwd = process.cwd();
const originalPath = process.env.PATH ?? '';

async function createTestRepo(): Promise<{ repoRoot: string; taskId: string }> {
	const repoRoot = await mkdtemp(path.join(os.tmpdir(), 'abb-work-runner-'));
	const binDir = path.join(repoRoot, 'bin');
	await mkdir(binDir, { recursive: true });
	await writeFile(
		path.join(binDir, 'codex'),
		[
			'#!/bin/sh',
			'out=""',
			'while [ "$#" -gt 0 ]; do',
			'  if [ "$1" = "--output-last-message" ]; then',
			'    out="$2"',
			'    shift 2',
			'    continue',
			'  fi',
			'  shift',
			'done',
			'prompt=$(cat)',
			'printf "%s\\n" "$prompt" > "$out"',
			'echo "fake codex run"',
		].join('\n'),
		'utf8',
	);
	await Bun.spawn(['chmod', '+x', path.join(binDir, 'codex')]).exited;

	await writeFile(
		path.join(repoRoot, 'WORKFLOW.md'),
		[
			'---',
			'base_branch: main',
			'task_branch_prefix: task/',
			'inbox_root: .agent-work/inbox',
			'worktree_root: .agent-work/worktrees',
			'run_root: .agent-work/runs',
			'lock_file: .agent-work/runner.lock.json',
			'codex_sandbox: workspace-write',
			'codex_approval: on-request',
			'---',
			'# Workloop',
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

	await mkdir(path.join(repoRoot, '.agent-work/inbox'), { recursive: true });
	const taskId = '0001-test-runner';
	await writeFile(
		path.join(repoRoot, '.agent-work/inbox', `${taskId}.md`),
		[
			'---',
			'title: Runner smoke task',
			'---',
			'## Goal',
			'Touch the runner path.',
			'',
			'## Constraints',
			'No durable archive.',
			'',
			'## Acceptance',
			'Leaves a handoff-ready run record.',
		].join('\n'),
		'utf8',
	);

	await initializeGitRepo(repoRoot);
	return { repoRoot, taskId };
}

async function initializeGitRepo(repoRoot: string): Promise<void> {
	await writeFile(path.join(repoRoot, 'README.md'), '# temp repo\n', 'utf8');
	spawnSync('git', ['init', '-b', 'main'], { cwd: repoRoot, stdio: 'ignore' });
	spawnSync('git', ['config', 'user.name', 'Test Runner'], { cwd: repoRoot, stdio: 'ignore' });
	spawnSync('git', ['config', 'user.email', 'test@example.com'], {
		cwd: repoRoot,
		stdio: 'ignore',
	});
	spawnSync('git', ['add', '.'], { cwd: repoRoot, stdio: 'ignore' });
	spawnSync('git', ['commit', '-m', 'init'], { cwd: repoRoot, stdio: 'ignore' });
}

beforeEach(() => {
	process.chdir(originalCwd);
	process.env.PATH = originalPath;
});

afterEach(() => {
	process.chdir(originalCwd);
	process.env.PATH = originalPath;
});

describe('work/runner', () => {
	test('runTaskById creates a temp branch/worktree and finishTask cleans it up', async () => {
		const { repoRoot, taskId } = await createTestRepo();
		process.chdir(repoRoot);
		process.env.PATH = `${path.join(repoRoot, 'bin')}:${originalPath}`;

		try {
			const runRecord = await runTaskById(taskId);
			expect(runRecord.status).toBe('handoff-ready');
			expect(runRecord.branchName).toBe(`task/${taskId}`);

			const finalMessage = await readFile(runRecord.finalMessagePath, 'utf8');
			expect(finalMessage).toContain(`Task ${taskId}`);

			await access(runRecord.worktreePath, fsConstants.F_OK);
			const cleanup = await finishTask(taskId, 'abandoned');
			expect(cleanup.removedTaskFile).toBe(true);
			expect(cleanup.removedRunDir).toBe(true);

			await expect(access(runRecord.worktreePath, fsConstants.F_OK)).rejects.toThrow();
			await expect(access(runRecord.runDir, fsConstants.F_OK)).rejects.toThrow();
		} finally {
			process.chdir(originalCwd);
			process.env.PATH = originalPath;
			await rm(repoRoot, { recursive: true, force: true });
		}
	});

	test('gcRunnerState removes stale lock, orphaned task branch, worktree, and run dir', async () => {
		const { repoRoot, taskId } = await createTestRepo();
		process.chdir(repoRoot);
		process.env.PATH = `${path.join(repoRoot, 'bin')}:${originalPath}`;

		try {
			const workflow = await loadWorkflowConfig(repoRoot);
			const workspace = await ensureTaskWorkspace(repoRoot, workflow, taskId);
			const runDir = path.join(repoRoot, workflow.runRoot, taskId);
			await mkdir(runDir, { recursive: true });
			await writeFile(path.join(runDir, 'run.json'), '{}\n', 'utf8');
			await rm(path.join(repoRoot, workflow.inboxRoot, `${taskId}.md`), { force: true });
			await writeFile(
				path.join(repoRoot, workflow.lockFile),
				JSON.stringify({ pid: 999_999, taskId, createdAt: new Date().toISOString() }),
				'utf8',
			);

			const result = await gcRunnerState();
			expect(result.removedLocks.length).toBe(1);
			expect(
				result.removedRunDirs.some((entry) => entry.endsWith(`/.agent-work/runs/${taskId}`)),
			).toBe(true);
			expect(result.removedBranches).toContain(getTaskBranchName(workflow, taskId));
			await expect(access(workspace.worktreePath, fsConstants.F_OK)).rejects.toThrow();
			await expect(access(runDir, fsConstants.F_OK)).rejects.toThrow();
		} finally {
			process.chdir(originalCwd);
			process.env.PATH = originalPath;
			await rm(repoRoot, { recursive: true, force: true });
		}
	});
});
