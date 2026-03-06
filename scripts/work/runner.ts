import { appendFile, mkdir, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import path from 'node:path';

import {
	listTaskFiles,
	loadTaskFile,
	loadWorkflowConfig,
	renderWorkflowPrompt,
	resolveTaskInput,
	type RunnerLock,
	type TaskFile,
	type WorkflowConfig,
} from './common';
import {
	branchExists,
	ensureTaskWorkspace,
	getTaskBranchName,
	getTaskFilePath,
	getTaskRunPath,
	isBranchMerged,
	listManagedTaskBranches,
	removeOrphanedWorktrees,
	removeTaskWorkspace,
} from './git';

type RunStatus = 'running' | 'handoff-ready' | 'failed';

type RunRecord = {
	taskId: string;
	taskTitle: string;
	taskFile: string;
	branchName: string;
	worktreePath: string;
	runDir: string;
	startedAt: string;
	finishedAt?: string;
	status: RunStatus;
	exitCode?: number | null;
	finalMessagePath: string;
	logPath: string;
};

type FinishDisposition = 'merged' | 'abandoned';

type GcResult = {
	removedRunDirs: string[];
	removedWorktrees: string[];
	removedBranches: string[];
	removedLocks: string[];
};

function repoRootFromCwd(): string {
	return process.cwd();
}

function resolveLockPath(repoRoot: string, workflow: WorkflowConfig): string {
	return path.join(repoRoot, workflow.lockFile);
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
		const raw = await readFile(lockPath, 'utf8');
		return JSON.parse(raw) as RunnerLock;
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
			const lock: RunnerLock = {
				pid: process.pid,
				taskId,
				createdAt: new Date().toISOString(),
			};
			await writeFile(lockPath, `${JSON.stringify(lock, null, 2)}\n`, {
				encoding: 'utf8',
				flag: 'wx',
			});
			return lockPath;
		} catch (error) {
			const existing = await readRunnerLock(lockPath);
			if (existing && (await isLivePid(existing.pid))) {
				throw new Error(
					`Task runner already holds an active lock for ${existing.taskId} (pid ${existing.pid}). Finish or kill that run before starting another.`,
				);
			}
			await rm(lockPath, { force: true });
			if (attempt === 1) {
				throw error;
			}
		}
	}

	throw new Error('Failed to acquire task runner lock.');
}

async function releaseRunnerLock(lockPath: string): Promise<void> {
	await rm(lockPath, { force: true });
}

async function writeRunRecord(runDir: string, record: RunRecord): Promise<void> {
	await mkdir(runDir, { recursive: true });
	await writeFile(path.join(runDir, 'run.json'), `${JSON.stringify(record, null, 2)}\n`, 'utf8');
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

async function runCodexTask(
	workflow: WorkflowConfig,
	worktreePath: string,
	prompt: string,
	runDir: string,
): Promise<{ exitCode: number | null; finalMessagePath: string; logPath: string }> {
	const finalMessagePath = path.join(runDir, 'final-message.md');
	const logPath = path.join(runDir, 'session.log');
	await writeFile(path.join(runDir, 'prompt.md'), `${prompt}\n`, 'utf8');

	const child = spawn('codex', createCodexArgs(workflow, worktreePath, finalMessagePath), {
		cwd: repoRootFromCwd(),
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

async function createRunContext(
	repoRoot: string,
	workflow: WorkflowConfig,
	task: TaskFile,
): Promise<RunRecord> {
	const { branchName, worktreePath } = await ensureTaskWorkspace(repoRoot, workflow, task.id);
	const runDir = getTaskRunPath(repoRoot, workflow, task.id);
	const startedAt = new Date().toISOString();
	const record: RunRecord = {
		taskId: task.id,
		taskTitle: task.title,
		taskFile: task.filePath,
		branchName,
		worktreePath,
		runDir,
		startedAt,
		status: 'running',
		finalMessagePath: path.join(runDir, 'final-message.md'),
		logPath: path.join(runDir, 'session.log'),
	};
	await writeRunRecord(runDir, record);
	return record;
}

async function completeRunRecord(
	record: RunRecord,
	exitCode: number | null,
	status: RunStatus,
): Promise<RunRecord> {
	const completed: RunRecord = {
		...record,
		exitCode,
		status,
		finishedAt: new Date().toISOString(),
	};
	await writeRunRecord(record.runDir, completed);
	return completed;
}

export async function runTaskById(input: string): Promise<RunRecord> {
	const repoRoot = repoRootFromCwd();
	const workflow = await loadWorkflowConfig(repoRoot);
	const task = await resolveTaskInput(repoRoot, workflow, input);
	const lockPath = await acquireRunnerLock(repoRoot, workflow, task.id);

	let runRecord: RunRecord | null = null;
	try {
		runRecord = await createRunContext(repoRoot, workflow, task);
		const prompt = renderWorkflowPrompt(workflow, task, repoRoot);
		const execution = await runCodexTask(
			workflow,
			runRecord.worktreePath,
			prompt,
			runRecord.runDir,
		);
		const status: RunStatus = execution.exitCode === 0 ? 'handoff-ready' : 'failed';
		runRecord = await completeRunRecord(runRecord, execution.exitCode, status);
		return runRecord;
	} catch (error) {
		if (runRecord) {
			await completeRunRecord(runRecord, 1, 'failed');
		}
		throw error;
	} finally {
		await releaseRunnerLock(lockPath);
	}
}

export async function runNextTask(): Promise<RunRecord> {
	const repoRoot = repoRootFromCwd();
	const workflow = await loadWorkflowConfig(repoRoot);
	const tasks = await listTaskFiles(repoRoot, workflow);
	if (tasks.length === 0) {
		throw new Error('No queued task files found in the work inbox.');
	}
	return runTaskById(tasks[0].id);
}

async function assertTaskNotLocked(
	repoRoot: string,
	workflow: WorkflowConfig,
	taskId: string,
): Promise<void> {
	const lock = await readRunnerLock(resolveLockPath(repoRoot, workflow));
	if (!lock) return;
	if (lock.taskId === taskId && !(await isLivePid(lock.pid))) {
		await rm(resolveLockPath(repoRoot, workflow), { force: true });
		return;
	}
	if (await isLivePid(lock.pid)) {
		throw new Error(`Cannot clean task ${taskId} while runner lock is active for ${lock.taskId}.`);
	}
}

export async function finishTask(
	taskId: string,
	disposition: FinishDisposition,
): Promise<{
	taskId: string;
	disposition: FinishDisposition;
	removedTaskFile: boolean;
	removedRunDir: boolean;
}> {
	const repoRoot = repoRootFromCwd();
	const workflow = await loadWorkflowConfig(repoRoot);
	await assertTaskNotLocked(repoRoot, workflow, taskId);

	const branchName = getTaskBranchName(workflow, taskId);
	if (disposition === 'merged' && branchExists(repoRoot, branchName)) {
		if (!isBranchMerged(repoRoot, branchName, workflow.baseBranch)) {
			throw new Error(
				`Temporary branch ${branchName} is not merged into ${workflow.baseBranch}. Finish with --abandoned or merge it before cleanup.`,
			);
		}
	}

	await removeTaskWorkspace(repoRoot, workflow, taskId);

	const taskFilePath = getTaskFilePath(repoRoot, workflow, taskId);
	let removedTaskFile = false;
	try {
		const loadedTask = await loadTaskFile(taskFilePath);
		if (loadedTask.id === taskId) {
			await rm(taskFilePath, { force: true });
			removedTaskFile = true;
		}
	} catch {
		// Missing task file is fine during cleanup.
	}

	const runDir = getTaskRunPath(repoRoot, workflow, taskId);
	let removedRunDir = false;
	try {
		await rm(runDir, { recursive: true, force: true });
		removedRunDir = true;
	} catch {
		removedRunDir = false;
	}

	return {
		taskId,
		disposition,
		removedTaskFile,
		removedRunDir,
	};
}

export async function gcRunnerState(): Promise<GcResult> {
	const repoRoot = repoRootFromCwd();
	const workflow = await loadWorkflowConfig(repoRoot);
	const lockPath = resolveLockPath(repoRoot, workflow);
	const removedLocks: string[] = [];
	const existingLock = await readRunnerLock(lockPath);
	if (existingLock && !(await isLivePid(existingLock.pid))) {
		await rm(lockPath, { force: true });
		removedLocks.push(lockPath);
	}

	const queuedTasks = new Set((await listTaskFiles(repoRoot, workflow)).map((task) => task.id));
	const removedRunDirs: string[] = [];
	const runRoot = path.join(repoRoot, workflow.runRoot);
	const runEntries = await readdir(runRoot, { withFileTypes: true }).catch(() => []);
	for (const entry of runEntries) {
		if (!entry.isDirectory()) continue;
		if (queuedTasks.has(entry.name)) continue;
		const target = path.join(runRoot, entry.name);
		await rm(target, { recursive: true, force: true });
		removedRunDirs.push(target);
	}

	const worktreeRoot = path.join(repoRoot, workflow.worktreeRoot);
	const removedWorktrees = await removeOrphanedWorktrees(repoRoot, worktreeRoot);

	const removedBranches: string[] = [];
	for (const branchName of listManagedTaskBranches(repoRoot, workflow)) {
		const taskId = branchName.slice(workflow.branchPrefix.length);
		if (queuedTasks.has(taskId)) continue;
		await removeTaskWorkspace(repoRoot, workflow, taskId);
		removedBranches.push(branchName);
	}

	return {
		removedRunDirs,
		removedWorktrees,
		removedBranches,
		removedLocks,
	};
}
