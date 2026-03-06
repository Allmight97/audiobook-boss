import { mkdir, readFile, readdir } from 'node:fs/promises';
import path from 'node:path';

export type CodexSandboxMode = 'read-only' | 'workspace-write' | 'danger-full-access';
export type CodexApprovalPolicy = 'untrusted' | 'on-request' | 'never' | 'on-failure';

export type WorkflowConfig = {
	filePath: string;
	promptTemplate: string;
	baseBranch: string;
	branchPrefix: string;
	inboxRoot: string;
	worktreeRoot: string;
	runRoot: string;
	lockFile: string;
	codexSandbox: CodexSandboxMode;
	codexApproval: CodexApprovalPolicy;
};

export type TaskFile = {
	id: string;
	filePath: string;
	title: string;
	goal: string;
	constraints: string;
	acceptance: string;
	context: string;
};

export type RunnerLock = {
	pid: number;
	taskId: string;
	createdAt: string;
};

function parseFrontMatter(raw: string): { frontMatter: Record<string, string>; body: string } {
	if (!raw.startsWith('---\n')) {
		return { frontMatter: {}, body: raw.trim() };
	}

	const endIndex = raw.indexOf('\n---\n', 4);
	if (endIndex === -1) {
		throw new Error('Front matter is missing a closing `---` marker.');
	}

	const frontMatterBlock = raw.slice(4, endIndex);
	const frontMatter: Record<string, string> = {};
	for (const line of frontMatterBlock.split('\n')) {
		const trimmed = line.trim();
		if (!trimmed || trimmed.startsWith('#')) continue;
		const separatorIndex = trimmed.indexOf(':');
		if (separatorIndex === -1) {
			throw new Error(`Invalid front matter line: ${trimmed}`);
		}
		const key = trimmed.slice(0, separatorIndex).trim();
		const value = trimmed.slice(separatorIndex + 1).trim();
		frontMatter[key] = value;
	}

	return {
		frontMatter,
		body: raw.slice(endIndex + 5).trim(),
	};
}

function requireEnumValue<T extends string>(
	value: string | undefined,
	allowed: readonly T[],
	fallback: T,
	fieldName: string,
): T {
	if (!value) return fallback;
	if (allowed.includes(value as T)) {
		return value as T;
	}
	throw new Error(`Invalid ${fieldName}: ${value}`);
}

export async function loadWorkflowConfig(repoRoot: string): Promise<WorkflowConfig> {
	const filePath = path.join(repoRoot, 'WORKFLOW.md');
	const raw = await readFile(filePath, 'utf8');
	const parsed = parseFrontMatter(raw);

	return {
		filePath,
		promptTemplate: parsed.body,
		baseBranch: parsed.frontMatter.base_branch ?? 'main',
		branchPrefix: parsed.frontMatter.task_branch_prefix ?? 'task/',
		inboxRoot: parsed.frontMatter.inbox_root ?? '.agent-work/inbox',
		worktreeRoot: parsed.frontMatter.worktree_root ?? '.agent-work/worktrees',
		runRoot: parsed.frontMatter.run_root ?? '.agent-work/runs',
		lockFile: parsed.frontMatter.lock_file ?? '.agent-work/runner.lock.json',
		codexSandbox: requireEnumValue(
			parsed.frontMatter.codex_sandbox,
			['read-only', 'workspace-write', 'danger-full-access'],
			'workspace-write',
			'codex_sandbox',
		),
		codexApproval: requireEnumValue(
			parsed.frontMatter.codex_approval,
			['untrusted', 'on-request', 'never', 'on-failure'],
			'on-request',
			'codex_approval',
		),
	};
}

function getSectionMap(body: string): Record<string, string> {
	const map: Record<string, string> = {};
	let activeKey: string | null = null;
	const buffer: string[] = [];

	const flush = () => {
		if (!activeKey) return;
		map[activeKey] = buffer.join('\n').trim();
		buffer.length = 0;
	};

	for (const line of body.split('\n')) {
		if (line.startsWith('## ')) {
			flush();
			activeKey = line.slice(3).trim().toLowerCase();
			continue;
		}
		if (activeKey) {
			buffer.push(line);
		}
	}
	flush();

	return map;
}

export async function loadTaskFile(filePath: string): Promise<TaskFile> {
	const raw = await readFile(filePath, 'utf8');
	const parsed = parseFrontMatter(raw);
	const sections = getSectionMap(parsed.body);
	const title = parsed.frontMatter.title;
	if (!title) {
		throw new Error(`Task file ${filePath} is missing front matter title.`);
	}

	const goal = sections.goal ?? '';
	const constraints = sections.constraints ?? '';
	const acceptance = sections.acceptance ?? '';
	const context = sections.context ?? '';

	if (!goal || !constraints || !acceptance) {
		throw new Error(
			`Task file ${filePath} must include ## Goal, ## Constraints, and ## Acceptance sections.`,
		);
	}

	return {
		id: path.basename(filePath, '.md'),
		filePath,
		title,
		goal,
		constraints,
		acceptance,
		context,
	};
}

export async function listTaskFiles(
	repoRoot: string,
	workflow: WorkflowConfig,
): Promise<TaskFile[]> {
	const inboxPath = path.join(repoRoot, workflow.inboxRoot);
	await mkdir(inboxPath, { recursive: true });
	const entries = await readdir(inboxPath, { withFileTypes: true });
	const taskFiles = entries
		.filter((entry) => entry.isFile() && entry.name.endsWith('.md'))
		.map((entry) => path.join(inboxPath, entry.name))
		.sort((left, right) => left.localeCompare(right));

	const tasks: TaskFile[] = [];
	for (const filePath of taskFiles) {
		tasks.push(await loadTaskFile(filePath));
	}
	return tasks;
}

export async function resolveTaskInput(
	repoRoot: string,
	workflow: WorkflowConfig,
	input: string,
): Promise<TaskFile> {
	const directPath = path.isAbsolute(input) ? input : path.join(repoRoot, input);
	try {
		return await loadTaskFile(directPath);
	} catch {
		// Fall back to inbox lookup by id.
	}

	const tasks = await listTaskFiles(repoRoot, workflow);
	const matched = tasks.find((task) => task.id === input);
	if (!matched) {
		throw new Error(`No queued task found for ${input}`);
	}
	return matched;
}

export function renderWorkflowPrompt(
	workflow: WorkflowConfig,
	task: TaskFile,
	repoRoot: string,
): string {
	const values: Record<string, string> = {
		'{{task.id}}': task.id,
		'{{task.title}}': task.title,
		'{{task.goal}}': task.goal,
		'{{task.constraints}}': task.constraints,
		'{{task.acceptance}}': task.acceptance,
		'{{task.context}}': task.context || 'No extra context supplied.',
		'{{repo.root}}': repoRoot,
		'{{workflow.base_branch}}': workflow.baseBranch,
	};

	let rendered = workflow.promptTemplate;
	for (const [token, value] of Object.entries(values)) {
		rendered = rendered.replaceAll(token, value);
	}

	if (/\{\{[^}]+\}\}/.test(rendered)) {
		throw new Error('WORKFLOW.md contains an unresolved placeholder token.');
	}

	return rendered.trim();
}
