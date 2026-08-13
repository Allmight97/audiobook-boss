import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import type { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import { afterAll, describe, expect, it } from 'vitest';

import {
	aaxcleanHelperBaseName,
	aaxcleanHelperTargetTriple,
	publishAaxcleanHelper,
	resolveAaxcleanHelperPaths,
} from './publish-aaxclean-helper';

const tempRoots: string[] = [];

function createHelperRepoFixture(): string {
	const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-aaxclean-publish-'));
	tempRoots.push(repoRoot);
	mkdirSync(path.join(repoRoot, 'tools/abb-aaxclean-helper/src/AbbAaxcleanHelper'), {
		recursive: true,
	});
	writeFileSync(
		path.join(repoRoot, 'tools/abb-aaxclean-helper/src/AbbAaxcleanHelper/Program.cs'),
		'// helper source fixture',
	);
	writeFileSync(
		path.join(repoRoot, 'tools/abb-aaxclean-helper/src/AbbAaxcleanHelper/AbbAaxcleanHelper.csproj'),
		'<Project />',
	);
	return repoRoot;
}

afterAll(() => {
	for (const tempRoot of tempRoots.splice(0, tempRoots.length)) {
		rmSync(tempRoot, { force: true, recursive: true });
	}
});

describe('publishAaxcleanHelper', () => {
	it('rejects a signal-terminated publish and does not copy leftover output', () => {
		const repoRoot = createHelperRepoFixture();
		const paths = resolveAaxcleanHelperPaths(repoRoot);
		mkdirSync(paths.publishDir, { recursive: true });
		writeFileSync(paths.publishedExecutablePath, 'old leftover helper');
		mkdirSync(paths.sidecarDir, { recursive: true });
		writeFileSync(paths.sidecarPath, 'previous sidecar');

		const commandRunner = (() => ({
			status: null,
			signal: 'SIGTERM',
		})) as typeof spawnSync;

		expect(() => publishAaxcleanHelper(repoRoot, { commandRunner, force: true })).toThrow(
			'AAXClean helper publish failed (signal SIGTERM)',
		);
		expect(existsSync(paths.publishedExecutablePath)).toBe(false);
		expect(readFileSync(paths.sidecarPath, 'utf8')).toBe('previous sidecar');
	});

	it('copies only a helper recreated by a successful publish', () => {
		const repoRoot = createHelperRepoFixture();
		const paths = resolveAaxcleanHelperPaths(repoRoot);
		mkdirSync(paths.publishDir, { recursive: true });
		writeFileSync(paths.publishedExecutablePath, 'old leftover helper');

		const commandRunner = ((command: string, args: string[]) => {
			expect(command).toContain('dotnet');
			expect(args[0]).toBe('publish');
			expect(existsSync(paths.publishedExecutablePath)).toBe(false);
			writeFileSync(paths.publishedExecutablePath, 'fresh helper');
			return { status: 0 };
		}) as typeof spawnSync;

		expect(publishAaxcleanHelper(repoRoot, { commandRunner, force: true })).toBe(paths.sidecarPath);
		expect(readFileSync(paths.sidecarPath, 'utf8')).toBe('fresh helper');
		expect(path.basename(paths.sidecarPath)).toBe(
			`${aaxcleanHelperBaseName}-${aaxcleanHelperTargetTriple}`,
		);
	});
});
