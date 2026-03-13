import { appendFile, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { spawn, spawnSync } from 'node:child_process';
import path from 'node:path';

import {
	loadWorkflowConfig,
	renderWorkflowPrompt,
	type RunnerLock,
	type TaskFile,
	type WorkflowConfig,
} from '../work/common';
import { ensureTaskWorkspace, getTaskRunPath, removeTaskWorkspace } from '../work/git';
import { parseGitHubIssueTask, type GitHubIssueTask } from './common';
import { commentOnIssue, createOrReusePr, fetchIssue } from './github';

type RunStatus = 'pr-opened' | 'local-complete' | 'failed';

type RunRecord = {
	issueNumber: number;
	issueTitle: string;
	issueUrl: string;
	deliveryMode: GitHubIssueTask['deliveryMode'];
	humanReview: GitHubIssueTask['humanReview'];
	branchName: string;
	worktreePath: string;
	runDir: string;
	startedAt: string;
	finishedAt?: string;
	status: RunStatus;
	exitCode?: number | null;
	finalMessagePath: string;
	logPath: string;
	changedPaths: string[];
	validationCommands: string[];
	prUrl?: string;
};

function repoRootFromCwd(): string {
	return process.cwd();
}

function resolveLockPath(repoRoot: string, workflow: WorkflowConfig): string {
	return path.join(repoRoot, workflow.lockFile);
}

function runGit(
	repoRoot: string,
	args: string[],
	options: { cwd?: string; allowFailure?: boolean } = {},
): { stdout: string; stderr: string; status: number | null } {
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

async function isLivePid(pid: number): Promise<boolean> {
	try {
		process.kill(pid, 0);
		return true;
	} catch {
		return false;
	}
}

async function readRunnerLock(lockPath: string): Promise<RunnerLock | null> {
	try {
		return JSON.parse(await readFile(lockPath, 'utf8')) as RunnerLock;
	} catch {
		return null;
	}
}

async function acquireRunnerLock(
	repoRoot: string,
	workflow: WorkflowConfig,
	taskId: string,
): Promise<string> {
	const lockPath = resolveLockPath(repoRoot, workflow);
	await mkdir(path.dirname(lockPath), { recursive: true });

	for (let attempt = 0; attempt < 2; attempt += 1) {
		try {
			const lock: RunnerLock = { pid: process.pid, taskId, createdAt: new Date().toISOString() };
			await writeFile(lockPath, `${JSON.stringify(lock, null, 2)}\n`, {
				encoding: 'utf8',
				flag: 'wx',
			});
			return lockPath;
		} catch (error) {
			const existing = await readRunnerLock(lockPath);
			if (existing && (await isLivePid(existing.pid))) {
				throw new Error(
					`Issue runner already holds an active lock for ${existing.taskId} (pid ${existing.pid}).`,
				);
			}
			await rm(lockPath, { force: true });
			if (attempt === 1) {
				throw error;
			}
		}
	}

	throw new Error('Failed to acquire issue runner lock.');
}

async function releaseRunnerLock(lockPath: string): Promise<void> {
	await rm(lockPath, { force: true });
}

async function appendStream(
	stream: NodeJS.ReadableStream | null,
	logPath: string,
	target: NodeJS.WriteStream,
): Promise<void> {
	if (!stream) return;
	for await (const chunk of stream) {
		const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(String(chunk));
		target.write(buffer);
		await appendFile(logPath, buffer);
	}
}

async function runLoggedCommand(
	command: string,
	args: string[],
	options: { cwd: string; logPath: string; env?: NodeJS.ProcessEnv },
): Promise<number | null> {
	const child = spawn(command, args, {
		cwd: options.cwd,
		stdio: ['ignore', 'pipe', 'pipe'],
		env: options.env,
	});
	const stdoutTask = appendStream(child.stdout, options.logPath, process.stdout);
	const stderrTask = appendStream(child.stderr, options.logPath, process.stderr);
	const exitCode = await new Promise<number | null>((resolve, reject) => {
		child.on('error', reject);
		child.on('close', (code) => resolve(code));
	});
	await Promise.all([stdoutTask, stderrTask]);
	return exitCode;
}

function buildCodexTask(issue: GitHubIssueTask): TaskFile {
	return {
		id: issue.runKey,
		filePath: issue.url,
		title: issue.title,
		goal: issue.goal,
		constraints: issue.constraints,
		acceptance: issue.acceptance,
		context: [
			issue.context,
			`GitHub issue: #${issue.number}`,
			`Issue URL: ${issue.url}`,
			`Validation contract:\n${issue.validation}`,
			`Delivery mode: ${issue.deliveryMode}`,
			`Human review: ${issue.humanReview}`,
		]
			.filter((entry) => entry && entry.trim().length > 0)
			.join('\n\n'),
	};
}

function createCodexArgs(
	workflow: WorkflowConfig,
	worktreePath: string,
	finalMessagePath: string,
): string[] {
	return [
		'exec',
		'-',
		'--cd',
		worktreePath,
		'--sandbox',
		workflow.codexSandbox,
		'--output-last-message',
		finalMessagePath,
		'-c',
		`approval_policy="${workflow.codexApproval}"`,
	];
}

async function runCodexIssue(
	workflow: WorkflowConfig,
	worktreePath: string,
	prompt: string,
	runDir: string,
): Promise<{ exitCode: number | null; finalMessagePath: string; logPath: string }> {
	const finalMessagePath = path.join(runDir, 'final-message.md');
	const logPath = path.join(runDir, 'session.log');
	await writeFile(path.join(runDir, 'prompt.md'), `${prompt}\n`, 'utf8');

	const child = spawn('codex', createCodexArgs(workflow, worktreePath, finalMessagePath), {
		cwd: worktreePath,
		stdio: ['pipe', 'pipe', 'pipe'],
	});
	child.stdin.write(prompt);
	child.stdin.end();

	const stdoutTask = appendStream(child.stdout, logPath, process.stdout);
	const stderrTask = appendStream(child.stderr, logPath, process.stderr);
	const exitCode = await new Promise<number | null>((resolve, reject) => {
		child.on('error', reject);
		child.on('close', (code) => resolve(code));
	});
	await Promise.all([stdoutTask, stderrTask]);

	return { exitCode, finalMessagePath, logPath };
}

function listChangedPaths(worktreePath: string): string[] {
	const output = runGit(worktreePath, ['status', '--porcelain', '--untracked-files=all'], {
		cwd: worktreePath,
	}).stdout;
	if (!output) {
		return [];
	}

	return output
		.split('\n')
		.map((line) => line.slice(3).trim())
		.filter((line) => line.length > 0)
		.map((entry) => (entry.includes(' -> ') ? (entry.split(' -> ').pop() ?? entry) : entry));
}

function isDocsOrControlplaneOnly(paths: string[]): boolean {
	const docsOnlyPattern =
		/^(docs\/project\.md$|README\.md$|AGENTS\.md$|WORKFLOW\.md$|CHANGELOG\.md$|\.github\/ISSUE_TEMPLATE\/|\.agents\/skills\/.+\.(md|yaml)$|\.agents\/settings\.local\.json$|scripts\/check-context-surface\.sh$)/;
	return paths.every((entry) => docsOnlyPattern.test(entry));
}

async function runValidation(
	_repoRoot: string,
	worktreePath: string,
	logPath: string,
	changedPaths: string[],
): Promise<string[]> {
	const commands = isDocsOrControlplaneOnly(changedPaths)
		? ['bash scripts/check-context-surface.sh', 'bun run test:controlplane']
		: ['scripts/checks.sh standard'];

	for (const command of commands) {
		const exitCode = await runLoggedCommand('bash', ['-lc', command], {
			cwd: worktreePath,
			logPath,
			env: process.env,
		});
		if (exitCode !== 0) {
			throw new Error(`Validation failed for command: ${command}`);
		}
	}

	return commands;
}

function commitMessage(issue: GitHubIssueTask): string {
	return `issue #${issue.number}: ${issue.title}`;
}

function prTitle(issue: GitHubIssueTask): string {
	return `#${issue.number} ${issue.title}`;
}

function prBody(issue: GitHubIssueTask, validationCommands: string[]): string {
	return [
		`Closes #${issue.number}`,
		'',
		'## Summary',
		'',
		'- Local GitHub issue runner completed the requested implementation.',
		'',
		'## Validation',
		'',
		...validationCommands.map((command) => `- \`${command}\``),
	].join('\n');
}

function issueCommentBody(record: RunRecord): string {
	const lines = [
		'## ABB Agent Run',
		'',
		`- Status: ${record.status}`,
		`- Branch: \`${record.branchName}\``,
		`- Delivery Mode: \`${record.deliveryMode}\``,
		`- Human Review: \`${record.humanReview}\``,
	];
	if (record.prUrl) {
		lines.push(`- PR: ${record.prUrl}`);
	}
	lines.push('- Validation:');
	for (const command of record.validationCommands) {
		lines.push(`  - \`${command}\``);
	}
	if (record.humanReview === 'visual') {
		lines.push('- Review Note: human visual review is still required before closing this issue.');
	}
	return lines.join('\n');
}

function failureCommentBody(issueNumber: number): string {
	return [
		'## ABB Agent Run',
		'',
		`- Status: failed locally before handoff for issue #${issueNumber}.`,
		'- Review Note: inspect the local runner logs on the operator machine before retrying.',
	].join('\n');
}

function createRunRecord(
	issue: GitHubIssueTask,
	branchName: string,
	worktreePath: string,
	runDir: string,
): RunRecord {
	return {
		issueNumber: issue.number,
		issueTitle: issue.title,
		issueUrl: issue.url,
		deliveryMode: issue.deliveryMode,
		humanReview: issue.humanReview,
		branchName,
		worktreePath,
		runDir,
		startedAt: new Date().toISOString(),
		status: 'failed',
		finalMessagePath: path.join(runDir, 'final-message.md'),
		logPath: path.join(runDir, 'session.log'),
		changedPaths: [],
		validationCommands: [],
	};
}

async function writeRunRecord(record: RunRecord): Promise<void> {
	await mkdir(record.runDir, { recursive: true });
	await writeFile(
		path.join(record.runDir, 'run.json'),
		`${JSON.stringify(record, null, 2)}\n`,
		'utf8',
	);
}

function prepareIssueWorkspace(
	repoRoot: string,
	workflow: WorkflowConfig,
	runKey: string,
): Promise<{ branchName: string; worktreePath: string }> {
	return (async () => {
		await removeTaskWorkspace(repoRoot, workflow, runKey);
		await rm(getTaskRunPath(repoRoot, workflow, runKey), { recursive: true, force: true });
		return ensureTaskWorkspace(repoRoot, workflow, runKey);
	})();
}

export async function runIssue(issueNumber: number): Promise<RunRecord> {
	const repoRoot = repoRootFromCwd();
	const workflow = await loadWorkflowConfig(repoRoot);
	const issue = parseGitHubIssueTask(fetchIssue(repoRoot, issueNumber));
	const lockPath = await acquireRunnerLock(repoRoot, workflow, issue.runKey);

	let record: RunRecord | null = null;
	try {
		const workspace = await prepareIssueWorkspace(repoRoot, workflow, issue.runKey);
		record = createRunRecord(
			issue,
			workspace.branchName,
			workspace.worktreePath,
			getTaskRunPath(repoRoot, workflow, issue.runKey),
		);
		await writeRunRecord(record);

		const prompt = renderWorkflowPrompt(workflow, buildCodexTask(issue), repoRoot);
		const codexRun = await runCodexIssue(workflow, record.worktreePath, prompt, record.runDir);
		record.finalMessagePath = codexRun.finalMessagePath;
		record.logPath = codexRun.logPath;
		record.exitCode = codexRun.exitCode;

		if (codexRun.exitCode !== 0) {
			throw new Error(`Codex exited with code ${codexRun.exitCode}`);
		}

		record.changedPaths = listChangedPaths(record.worktreePath);
		if (record.changedPaths.length === 0) {
			throw new Error(`Issue #${issue.number} completed without any working tree changes.`);
		}

		record.validationCommands = await runValidation(
			repoRoot,
			record.worktreePath,
			record.logPath,
			record.changedPaths,
		);

		runGit(repoRoot, ['add', '-A'], { cwd: record.worktreePath });
		runGit(repoRoot, ['commit', '-m', commitMessage(issue)], { cwd: record.worktreePath });

		if (issue.deliveryMode === 'pr') {
			runGit(repoRoot, ['push', '-u', 'origin', record.branchName], { cwd: record.worktreePath });
			const pr = await createOrReusePr(repoRoot, {
				title: prTitle(issue),
				body: prBody(issue, record.validationCommands),
				headBranch: record.branchName,
				baseBranch: workflow.baseBranch,
			});
			record.prUrl = pr.url;
			record.status = 'pr-opened';
		} else {
			record.status = 'local-complete';
		}

		record.finishedAt = new Date().toISOString();
		await writeRunRecord(record);
		await commentOnIssue(repoRoot, issue.number, issueCommentBody(record));
		return record;
	} catch (error) {
		if (record) {
			record.status = 'failed';
			record.finishedAt = new Date().toISOString();
			await writeRunRecord(record);
			await commentOnIssue(repoRoot, issueNumber, failureCommentBody(issueNumber)).catch(() => {});
		}
		throw error;
	} finally {
		await releaseRunnerLock(lockPath);
	}
}
