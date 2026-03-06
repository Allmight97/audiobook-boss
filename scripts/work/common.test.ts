import { describe, expect, test } from 'bun:test';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

import { loadTaskFile, loadWorkflowConfig, renderWorkflowPrompt } from './common';

describe('work/common', () => {
	test('loads workflow config and renders the prompt template', async () => {
		const repoRoot = await mkdtemp(path.join(os.tmpdir(), 'abb-work-common-'));
		try {
			await writeFile(
				path.join(repoRoot, 'WORKFLOW.md'),
				[
					'---',
					'base_branch: trunk',
					'task_branch_prefix: lane/',
					'inbox_root: .queue/inbox',
					'worktree_root: .queue/worktrees',
					'run_root: .queue/runs',
					'lock_file: .queue/runner.lock.json',
					'codex_sandbox: workspace-write',
					'codex_approval: on-request',
					'---',
					'Task {{task.id}} :: {{task.title}}',
					'Goal: {{task.goal}}',
					'Constraints: {{task.constraints}}',
					'Acceptance: {{task.acceptance}}',
					'Context: {{task.context}}',
					'Repo: {{repo.root}}',
					'Base: {{workflow.base_branch}}',
				].join('\n'),
				'utf8',
			);

			const taskPath = path.join(repoRoot, '.queue/inbox/0001-test-task.md');
			await mkdir(path.dirname(taskPath), { recursive: true });
			await writeFile(
				taskPath,
				[
					'---',
					'title: Test task',
					'---',
					'## Goal',
					'Ship the thing.',
					'',
					'## Constraints',
					'Keep it tight.',
					'',
					'## Acceptance',
					'It works.',
					'',
					'## Context',
					'Extra notes.',
				].join('\n'),
				'utf8',
			);

			const workflow = await loadWorkflowConfig(repoRoot);
			const task = await loadTaskFile(taskPath);
			const prompt = renderWorkflowPrompt(workflow, task, repoRoot);

			expect(workflow.baseBranch).toBe('trunk');
			expect(workflow.branchPrefix).toBe('lane/');
			expect(task.id).toBe('0001-test-task');
			expect(prompt).toContain('Task 0001-test-task :: Test task');
			expect(prompt).toContain(`Repo: ${repoRoot}`);
			expect(prompt).toContain('Base: trunk');
		} finally {
			await rm(repoRoot, { recursive: true, force: true });
		}
	});
});
