import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

type GhResult = {
	stdout: string;
	stderr: string;
	status: number | null;
};

export type GitHubIssueView = {
	number: number;
	title: string;
	body: string;
	url: string;
	labels: string[];
};

export type GitHubPrSummary = {
	number: number;
	url: string;
};

function runGh(
	repoRoot: string,
	args: string[],
	options: { allowFailure?: boolean } = {},
): GhResult {
	const ghCommand = process.env.GH_BIN?.trim() || 'gh';
	const result = spawnSync(ghCommand, args, {
		cwd: repoRoot,
		encoding: 'utf8',
	});
	const command = `${ghCommand} ${args.join(' ')}`;
	if (!options.allowFailure && result.status !== 0) {
		throw new Error(result.stderr.trim() || `${command} failed`);
	}
	return {
		stdout: result.stdout.trim(),
		stderr: result.stderr.trim(),
		status: result.status,
	};
}

async function withBodyFile<T>(
	content: string,
	callback: (bodyFile: string) => Promise<T>,
): Promise<T> {
	const dir = await mkdtemp(path.join(os.tmpdir(), 'abb-gh-body-'));
	const bodyFile = path.join(dir, 'body.md');
	await writeFile(bodyFile, content, 'utf8');
	try {
		return await callback(bodyFile);
	} finally {
		await rm(dir, { recursive: true, force: true });
	}
}

export function fetchIssue(repoRoot: string, issueNumber: number): GitHubIssueView {
	const result = runGh(repoRoot, [
		'issue',
		'view',
		String(issueNumber),
		'--json',
		'number,title,body,url,labels',
	]);
	const parsed = JSON.parse(result.stdout) as {
		number: number;
		title: string;
		body: string;
		url: string;
		labels: Array<{ name: string }>;
	};

	return {
		number: parsed.number,
		title: parsed.title,
		body: parsed.body,
		url: parsed.url,
		labels: parsed.labels.map((label) => label.name),
	};
}

export async function createIssue(
	repoRoot: string,
	options: { title: string; body: string; labels: string[] },
): Promise<{ number: number; url: string }> {
	return withBodyFile(options.body, async (bodyFile) => {
		const args = ['issue', 'create', '--title', options.title, '--body-file', bodyFile];
		for (const label of options.labels) {
			args.push('--label', label);
		}
		const result = runGh(repoRoot, args);
		const url = result.stdout.trim().split('\n').pop()?.trim();
		if (!url) {
			throw new Error('gh issue create did not return an issue URL.');
		}
		const number = Number(url.split('/').pop());
		if (!Number.isFinite(number)) {
			throw new Error(`Unable to parse issue number from URL: ${url}`);
		}
		return { number, url };
	});
}

export async function commentOnIssue(
	repoRoot: string,
	issueNumber: number,
	body: string,
): Promise<void> {
	await withBodyFile(body, async (bodyFile) => {
		runGh(repoRoot, ['issue', 'comment', String(issueNumber), '--body-file', bodyFile]);
	});
}

export async function createOrReusePr(
	repoRoot: string,
	options: { title: string; body: string; headBranch: string; baseBranch: string },
): Promise<GitHubPrSummary> {
	return withBodyFile(options.body, async (bodyFile) => {
		const create = runGh(
			repoRoot,
			[
				'pr',
				'create',
				'--title',
				options.title,
				'--body-file',
				bodyFile,
				'--head',
				options.headBranch,
				'--base',
				options.baseBranch,
			],
			{ allowFailure: true },
		);
		if (create.status === 0) {
			const url = create.stdout.trim().split('\n').pop()?.trim();
			if (!url) {
				throw new Error('gh pr create did not return a PR URL.');
			}
			return { number: Number(url.split('/').pop()), url };
		}

		const existing = runGh(repoRoot, [
			'pr',
			'list',
			'--head',
			options.headBranch,
			'--state',
			'open',
			'--json',
			'number,url',
		]);
		const parsed = JSON.parse(existing.stdout) as Array<{ number: number; url: string }>;
		if (parsed.length > 0) {
			return parsed[0];
		}

		throw new Error(create.stderr || 'Failed to create or reuse PR.');
	});
}

export async function readBodyFile(filePath: string): Promise<string> {
	return (await readFile(filePath, 'utf8')).trim();
}
