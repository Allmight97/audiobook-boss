import {
	chmodSync,
	copyFileSync,
	mkdtempSync,
	mkdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { afterEach, describe, expect, it } from 'bun:test';

const tempRoots: string[] = [];
const REPO_ROOT = path.resolve(import.meta.dir, '..');

function runOrThrow(
	command: string,
	args: string[],
	options: { cwd: string; env?: NodeJS.ProcessEnv },
): string {
	const result = spawnSync(command, args, {
		cwd: options.cwd,
		encoding: 'utf8',
		env: options.env,
	});
	if (result.status !== 0) {
		throw new Error(
			[
				`Command failed: ${command} ${args.join(' ')}`,
				`exit=${String(result.status)}`,
				result.stdout,
				result.stderr,
			]
				.filter(Boolean)
				.join('\n'),
		);
	}
	return result.stdout;
}

function appendCommit(repoRoot: string, fileName: string, contents: string, message: string): void {
	writeFileSync(path.join(repoRoot, fileName), contents);
	runOrThrow('git', ['add', fileName], { cwd: repoRoot });
	runOrThrow('git', ['commit', '-m', message], { cwd: repoRoot });
}

function createReleaseFixture(): { repoRoot: string } {
	const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-release-'));
	tempRoots.push(repoRoot);

	mkdirSync(path.join(repoRoot, 'scripts'), { recursive: true });
	mkdirSync(path.join(repoRoot, 'bin'), { recursive: true });

	runOrThrow('git', ['init'], { cwd: repoRoot });
	runOrThrow('git', ['config', 'user.name', 'Codex Test'], { cwd: repoRoot });
	runOrThrow('git', ['config', 'user.email', 'codex@example.com'], { cwd: repoRoot });

	writeFileSync(
		path.join(repoRoot, 'package.json'),
		JSON.stringify({ name: 'audiobook-boss', version: '1.0.4' }, null, 2),
	);
	writeFileSync(
		path.join(repoRoot, 'CHANGELOG.md'),
		[
			'# Changelog',
			'',
			'## [Unreleased]',
			'',
			'## [1.0.5] - 2026-04-16',
			'',
			'### Changed',
			'',
			'- Test fixture release entry.',
			'',
		].join('\n'),
	);
	writeFileSync(path.join(repoRoot, 'commit-log.txt'), '');
	writeFileSync(
		path.join(repoRoot, 'scripts', 'bump-version.sh'),
		'#!/usr/bin/env bash\nprintf "bump:%s\\n" "$1" >> .stub-log\nexit 0\n',
	);
	writeFileSync(
		path.join(repoRoot, 'bin', 'bun'),
		`#!/usr/bin/env bash
printf "bun:%s %s\n" "\${1:-}" "\${2:-}" >> .stub-log
exit 0
`,
	);
	chmodSync(path.join(repoRoot, 'scripts', 'bump-version.sh'), 0o755);
	chmodSync(path.join(repoRoot, 'bin', 'bun'), 0o755);

	copyFileSync(
		path.join(REPO_ROOT, 'scripts', 'release.sh'),
		path.join(repoRoot, 'scripts', 'release.sh'),
	);
	chmodSync(path.join(repoRoot, 'scripts', 'release.sh'), 0o755);

	runOrThrow('git', ['add', '.'], { cwd: repoRoot });
	runOrThrow('git', ['commit', '-m', 'chore: initial fixture'], { cwd: repoRoot });
	runOrThrow('git', ['tag', 'v1.0.4'], { cwd: repoRoot });

	for (let index = 0; index < 16; index += 1) {
		appendCommit(
			repoRoot,
			'commit-log.txt',
			`${readFileSync(path.join(repoRoot, 'commit-log.txt'), 'utf8')}${index}\n`,
			`chore: history ${index + 1}`,
		);
	}

	return { repoRoot };
}

afterEach(() => {
	for (const tempRoot of tempRoots.splice(0, tempRoots.length)) {
		rmSync(tempRoot, { force: true, recursive: true });
	}
});

describe('release.sh', () => {
	it('survives the preview step with more than 15 commits after the last tag', () => {
		const { repoRoot } = createReleaseFixture();

		const result = spawnSync(
			'bash',
			['scripts/release.sh', '--version', '1.0.5', '--changelog-verified', '--no-commit-tag'],
			{
				cwd: repoRoot,
				encoding: 'utf8',
				env: {
					...process.env,
					PATH: `${path.join(repoRoot, 'bin')}:${process.env.PATH ?? ''}`,
				},
			},
		);

		expect(result.status).toBe(0);
		expect(result.stdout).toContain('Changes since last tag:');
		expect(result.stdout).toContain('Skipped commit. To finish manually:');
		expect(result.stderr).toBe('');
		expect(readFileSync(path.join(repoRoot, '.stub-log'), 'utf8')).toContain('bump:1.0.5');
		expect(readFileSync(path.join(repoRoot, '.stub-log'), 'utf8')).toContain('bun:run app:build');
	});
});
