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
import { describe, expect, it } from 'bun:test';

import { resolveReleaseDmgArtifact } from './resolve-release-dmg';

const SYSTEM_BASH = '/bin/bash';
const SYSTEM_GIT = '/usr/bin/git';
const TEST_PATH = '/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin';
const REPO_ROOT = path.resolve(import.meta.dir, '..');
const TEST_ENV = {
	HOME: process.env.HOME ?? os.tmpdir(),
	PATH: TEST_PATH,
	TMPDIR: process.env.TMPDIR ?? os.tmpdir(),
};

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

function appendEmptyHistory(repoRoot: string, commitCount: number): void {
	for (const index of Array.from({ length: commitCount }, (_, currentIndex) => currentIndex)) {
		const commitNumber = String(index + 1).padStart(2, '0');
		runOrThrow(
			SYSTEM_GIT,
			['commit', '--allow-empty', '--quiet', '-m', `chore: history ${commitNumber}`],
			{
				cwd: repoRoot,
			},
		);
	}
}

function createReleaseFixture(): { repoRoot: string } {
	const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-release-'));

	mkdirSync(path.join(repoRoot, 'scripts'), { recursive: true });
	mkdirSync(path.join(repoRoot, 'scripts', 'lib'), { recursive: true });
	mkdirSync(path.join(repoRoot, 'src-tauri'), { recursive: true });
	mkdirSync(path.join(repoRoot, 'bin'), { recursive: true });

	runOrThrow(SYSTEM_GIT, ['init'], { cwd: repoRoot, env: { ...process.env, ...TEST_ENV } });
	runOrThrow(SYSTEM_GIT, ['config', 'user.name', 'Codex Test'], { cwd: repoRoot });
	runOrThrow(SYSTEM_GIT, ['config', 'user.email', 'codex@example.com'], { cwd: repoRoot });

	writeFileSync(
		path.join(repoRoot, 'package.json'),
		JSON.stringify({ name: 'audiobook-boss', version: '1.0.4' }, null, 2),
	);
	writeFileSync(
		path.join(repoRoot, 'src-tauri', 'tauri.conf.json'),
		JSON.stringify({ productName: 'AudioBook Boss' }, null, 2),
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
	writeFileSync(
		path.join(repoRoot, 'scripts', 'bump-version.sh'),
		'#!/bin/bash\nprintf "bump:%s\\n" "$1" >> .stub-log\nexit 0\n',
	);
	copyFileSync(
		path.join(REPO_ROOT, 'scripts', 'lib', 'release-common.sh'),
		path.join(repoRoot, 'scripts', 'lib', 'release-common.sh'),
	);
	writeFileSync(
		path.join(repoRoot, 'bin', 'bun'),
		`#!/bin/bash
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

	runOrThrow(SYSTEM_GIT, ['add', '.'], { cwd: repoRoot });
	runOrThrow(SYSTEM_GIT, ['commit', '-m', 'chore: initial fixture'], { cwd: repoRoot });
	runOrThrow(SYSTEM_GIT, ['tag', 'v1.0.4'], { cwd: repoRoot });

	appendEmptyHistory(repoRoot, 128);

	return { repoRoot };
}

describe('release.sh', () => {
	it('preflights the dmg release artifact before tagging', () => {
		const { repoRoot } = createReleaseFixture();

		try {
			const oldPreview = spawnSync(
				SYSTEM_BASH,
				['-lc', 'set -euo pipefail; git log --oneline --no-decorate v1.0.4..HEAD | head -15'],
				{
					cwd: repoRoot,
					encoding: 'utf8',
					env: {
						...process.env,
						PATH: TEST_PATH,
					},
				},
			);

			expect(oldPreview.status).not.toBe(0);

			const outputRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-release-output-'));
			const stdoutPath = path.join(outputRoot, 'stdout.log');
			const stderrPath = path.join(outputRoot, 'stderr.log');

			try {
				const result = spawnSync(
					SYSTEM_BASH,
					[
						'-c',
						`./scripts/release.sh --version 1.0.5 --changelog-verified --no-commit-tag > "${stdoutPath}" 2> "${stderrPath}"`,
					],
					{
						cwd: repoRoot,
						encoding: 'utf8',
						env: {
							...process.env,
							PATH: `${path.join(repoRoot, 'bin')}:${TEST_PATH}`,
						},
					},
				);
				const stdout = readFileSync(stdoutPath, 'utf8');
				const stderr = readFileSync(stderrPath, 'utf8');
				if (result.status !== 0) {
					console.log(JSON.stringify({ status: result.status, stdout, stderr }, null, 2));
				}

				expect(result.status).toBe(0);
				expect(stdout).toContain('Changes since last tag:');
				expect(stdout).toContain('Skipped commit. To finish manually:');
				expect(stderr).toBe('');
				expect(readFileSync(path.join(repoRoot, '.stub-log'), 'utf8')).toContain('bump:1.0.5');
				expect(readFileSync(path.join(repoRoot, '.stub-log'), 'utf8')).toContain(
					'bun:run app:build:dmg',
				);
			} finally {
				rmSync(outputRoot, { force: true, recursive: true });
			}
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	}, 30000);
});

describe('resolveReleaseDmgArtifact', () => {
	it('resolves the single version-matching dmg artifact', () => {
		const { repoRoot } = createReleaseFixture();

		try {
			const bundleDir = path.join(repoRoot, 'target', 'release', 'bundle', 'dmg');
			mkdirSync(bundleDir, { recursive: true });
			const dmgPath = path.join(bundleDir, 'AudioBook Boss_1.0.5_aarch64.dmg');
			writeFileSync(dmgPath, '');

			expect(resolveReleaseDmgArtifact(repoRoot, '1.0.5')).toBe(
				path.join(bundleDir, 'AudioBook Boss_1.0.5_aarch64.dmg'),
			);
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('fails when multiple dmg artifacts match the tagged version', () => {
		const { repoRoot } = createReleaseFixture();

		try {
			const bundleDir = path.join(repoRoot, 'target', 'release', 'bundle', 'dmg');
			mkdirSync(bundleDir, { recursive: true });
			writeFileSync(path.join(bundleDir, 'AudioBook Boss_1.0.5_aarch64.dmg'), '');
			writeFileSync(path.join(bundleDir, 'AudioBook Boss_1.0.5_x64.dmg'), '');

			expect(() => resolveReleaseDmgArtifact(repoRoot, '1.0.5')).toThrow(
				'Expected exactly one DMG for release 1.0.5, found 2.',
			);
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});
});
