import { mkdir, readdir, rm, stat } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import path from 'node:path';

import type { WorkflowConfig } from './common';

type GitCommandResult = {
	stdout: string;
	stderr: string;
	status: number | null;
};

type ManagedWorktree = {
	worktreePath: string;
	branchName: string | null;
};

function runGit(
	repoRoot: string,
	args: string[],
	options: { cwd?: string; allowFailure?: boolean } = {},
): GitCommandResult {
	const result = spawnSync('git', args, {
		cwd: options.cwd ?? repoRoot,
		encoding: 'utf8',
	});
	const command = `git ${args.join(' ')}`;
	if (!options.allowFailure && result.status !== 0) {
		throw new Error(result.stderr.trim() || `${command} failed`);
	}
	return {
		stdout: result.stdout.trim(),
		stderr: result.stderr.trim(),
		status: result.status,
	};
}

export function getTaskBranchName(workflow: WorkflowConfig, taskId: string): string {
	return `${workflow.branchPrefix}${taskId}`;
}

export function getTaskWorktreePath(
	repoRoot: string,
	workflow: WorkflowConfig,
	taskId: string,
): string {
	return path.join(repoRoot, workflow.worktreeRoot, taskId);
}

export function getTaskRunPath(repoRoot: string, workflow: WorkflowConfig, taskId: string): string {
	return path.join(repoRoot, workflow.runRoot, taskId);
}

export function getTaskFilePath(
	repoRoot: string,
	workflow: WorkflowConfig,
	taskId: string,
): string {
	return path.join(repoRoot, workflow.inboxRoot, `${taskId}.md`);
}

export function branchExists(repoRoot: string, branchName: string): boolean {
	return (
		runGit(repoRoot, ['rev-parse', '--verify', '--quiet', branchName], {
			allowFailure: true,
		}).status === 0
	);
}

export function isBranchMerged(repoRoot: string, branchName: string, baseBranch: string): boolean {
	if (!branchExists(repoRoot, branchName)) {
		return false;
	}
	return (
		runGit(repoRoot, ['merge-base', '--is-ancestor', branchName, baseBranch], {
			allowFailure: true,
		}).status === 0
	);
}

export function listManagedTaskBranches(repoRoot: string, workflow: WorkflowConfig): string[] {
	const output = runGit(repoRoot, ['branch', '--format', '%(refname:short)']).stdout;
	const prefix = workflow.branchPrefix;
	return output
		.split('\n')
		.map((entry) => entry.trim())
		.filter((entry) => entry.length > 0 && entry.startsWith(prefix));
}

export function listManagedWorktrees(repoRoot: string): ManagedWorktree[] {
	const output = runGit(repoRoot, ['worktree', 'list', '--porcelain']).stdout;
	if (!output) {
		return [];
	}

	const entries = output.split('\n\n').filter((entry) => entry.trim().length > 0);
	return entries.map((entry) => {
		const lines = entry.split('\n');
		const worktreeLine = lines.find((line) => line.startsWith('worktree '));
		const branchLine = lines.find((line) => line.startsWith('branch '));
		if (!worktreeLine) {
			throw new Error('Malformed `git worktree list --porcelain` output.');
		}
		return {
			worktreePath: worktreeLine.slice('worktree '.length),
			branchName: branchLine ? branchLine.slice('branch refs/heads/'.length) : null,
		};
	});
}

async function pathExists(targetPath: string): Promise<boolean> {
	try {
		await stat(targetPath);
		return true;
	} catch {
		return false;
	}
}

export async function ensureTaskWorkspace(
	repoRoot: string,
	workflow: WorkflowConfig,
	taskId: string,
): Promise<{ branchName: string; worktreePath: string }> {
	const branchName = getTaskBranchName(workflow, taskId);
	const worktreePath = getTaskWorktreePath(repoRoot, workflow, taskId);

	runGit(repoRoot, ['rev-parse', '--verify', '--quiet', workflow.baseBranch]);

	if (branchExists(repoRoot, branchName)) {
		throw new Error(
			`Temporary task branch ${branchName} already exists. Finish or clean the task before rerunning it.`,
		);
	}

	if (await pathExists(worktreePath)) {
		throw new Error(
			`Worktree path ${worktreePath} already exists. Run \`bun run work:gc\` or \`bun run work:finish --task ${taskId} --abandoned\` before rerunning the task.`,
		);
	}

	await mkdir(path.dirname(worktreePath), { recursive: true });
	runGit(repoRoot, ['worktree', 'add', '-b', branchName, worktreePath, workflow.baseBranch]);
	return { branchName, worktreePath };
}

export async function removeTaskWorkspace(
	repoRoot: string,
	workflow: WorkflowConfig,
	taskId: string,
): Promise<void> {
	const branchName = getTaskBranchName(workflow, taskId);
	const worktreePath = getTaskWorktreePath(repoRoot, workflow, taskId);
	const registeredWorktrees = listManagedWorktrees(repoRoot);
	const isRegistered = registeredWorktrees.some((entry) => entry.worktreePath === worktreePath);

	if (isRegistered) {
		runGit(repoRoot, ['worktree', 'remove', '--force', worktreePath], { allowFailure: true });
	} else {
		await rm(worktreePath, { recursive: true, force: true });
	}

	if (branchExists(repoRoot, branchName)) {
		runGit(repoRoot, ['branch', '-D', branchName], { allowFailure: true });
	}
}

export async function removeOrphanedWorktrees(
	repoRoot: string,
	repoManagedRoot: string,
): Promise<string[]> {
	const removed: string[] = [];
	const entries = await readdir(repoManagedRoot, { withFileTypes: true }).catch(() => []);
	for (const entry of entries) {
		if (!entry.isDirectory()) continue;
		const fullPath = path.join(repoManagedRoot, entry.name);
		const registered = listManagedWorktrees(repoRoot).some(
			(candidate) => candidate.worktreePath === fullPath,
		);
		if (!registered) {
			await rm(fullPath, { recursive: true, force: true });
			removed.push(fullPath);
		}
	}
	return removed;
}
