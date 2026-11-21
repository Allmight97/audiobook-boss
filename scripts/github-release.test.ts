import { execFileSync, spawnSync } from 'node:child_process';
import { chmodSync, existsSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

import { describe, expect, it } from 'vitest';

const repo = path.resolve(import.meta.dirname, '..');
const head = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repo, encoding: 'utf8' }).trim();

function ghDir(): string {
	const dir = mkdtempSync(path.join(tmpdir(), 'abb-gh-'));
	writeFileSync(
		path.join(dir, 'gh'),
		[
			'#!/bin/bash',
			'set -euo pipefail',
			'if [[ "${1:-}" == api ]]; then',
			"	printf '%s\\n' 'gh: Not Found (HTTP 404)' >&2",
			'	exit 1',
			'fi',
			'if [[ "${1:-}" == release && "${2:-}" == view ]]; then',
			'	if [[ "${FAKE_GH_MISSING:-}" == 1 ]]; then',
			"		printf '%s\\n' 'release not found' >&2",
			'		exit 1',
			'	fi',
			'	cat "$FAKE_GH_JSON"',
			'	exit 0',
			'fi',
			'printf \'%s\\n\' "unexpected gh $*" >&2',
			'exit 1',
			'',
		].join('\n'),
	);
	chmodSync(path.join(dir, 'gh'), 0o755);
	return dir;
}

function plan(extra: Record<string, string>) {
	const output = path.join(mkdtempSync(path.join(tmpdir(), 'abb-out-')), 'output');
	const result = spawnSync('bash', ['scripts/github-release.sh', 'plan'], {
		cwd: repo,
		encoding: 'utf8',
		env: {
			...process.env,
			PATH: `${ghDir()}:${process.env.PATH ?? ''}`,
			GITHUB_REPOSITORY: 'Allmight97/audiobook-boss',
			GITHUB_SHA: head,
			GITHUB_OUTPUT: output,
			...extra,
		},
	});
	return {
		status: result.status,
		stdout: result.stdout,
		stderr: result.stderr,
		output: existsSync(output) ? readFileSync(output, 'utf8') : '',
	};
}

function releaseJson(target: string): string {
	const file = path.join(mkdtempSync(path.join(tmpdir(), 'abb-json-')), 'release.json');
	writeFileSync(file, JSON.stringify({ isDraft: true, assets: [], targetCommitish: target }));
	return file;
}

describe('github-release.sh plan', () => {
	it('finishes a draft that targets this commit and has no tag yet', () => {
		const result = plan({ FAKE_GH_JSON: releaseJson(head) });
		expect(result.status).toBe(0);
		expect(result.stdout).toContain('Finishing that draft');
		expect(result.output).toBe('publish=true\n');
	});

	it('refuses a draft that targets another commit', () => {
		const result = plan({
			FAKE_GH_JSON: releaseJson('0123456789abcdef0123456789abcdef01234567'),
		});
		expect(result.status).toBe(1);
		expect(result.stderr).toContain('Fix: delete the draft, then rerun.');
	});
});
