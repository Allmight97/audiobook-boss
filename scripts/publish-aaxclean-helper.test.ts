import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import type { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import { afterAll, describe, expect, it } from 'vitest';

import {
	aaxcleanHelperBaseName,
	aaxcleanHelperPublishTarget,
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
	writeFileSync(
		path.join(repoRoot, 'tools/abb-aaxclean-helper/global.json'),
		JSON.stringify({ sdk: { version: '10.0.401', rollForward: 'latestPatch' } }),
	);
	return repoRoot;
}

afterAll(() => {
	for (const tempRoot of tempRoots.splice(0, tempRoots.length)) {
		rmSync(tempRoot, { force: true, recursive: true });
	}
});

describe('aaxcleanHelperPublishTarget', () => {
	it('maps macOS Apple Silicon, Linux x64, and Linux arm64', () => {
		expect(aaxcleanHelperPublishTarget('darwin', 'arm64')).toEqual({
			rid: 'osx-arm64',
			triple: 'aarch64-apple-darwin',
		});
		expect(aaxcleanHelperPublishTarget('linux', 'x64')).toEqual({
			rid: 'linux-x64',
			triple: 'x86_64-unknown-linux-gnu',
		});
		expect(aaxcleanHelperPublishTarget('linux', 'arm64')).toEqual({
			rid: 'linux-arm64',
			triple: 'aarch64-unknown-linux-gnu',
		});
	});

	it('refuses hosts the helper does not publish for', () => {
		expect(() => aaxcleanHelperPublishTarget('win32', 'x64')).toThrow(
			'AAXClean helper publish is not supported on win32/x64',
		);
		expect(() => aaxcleanHelperPublishTarget('darwin', 'x64')).toThrow(
			'AAXClean helper publish is not supported on darwin/x64',
		);
	});
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
			expect(args).toEqual([
				'publish',
				paths.projectPath,
				'-c',
				'Release',
				'-r',
				paths.rid,
				'-o',
				paths.publishDir,
			]);
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

	it('republishes when the sidecar is a leftover stub', () => {
		const repoRoot = createHelperRepoFixture();
		const paths = resolveAaxcleanHelperPaths(repoRoot);
		mkdirSync(paths.sidecarDir, { recursive: true });
		writeFileSync(paths.sidecarPath, '#!/usr/bin/env sh\nexit 0\n');

		const commandRunner = ((_command: string, args: string[]) => {
			const outputDir = args[args.indexOf('-o') + 1] as string;
			mkdirSync(outputDir, { recursive: true });
			writeFileSync(path.join(outputDir, aaxcleanHelperBaseName), 'real helper');
			return { status: 0 };
		}) as typeof spawnSync;

		expect(publishAaxcleanHelper(repoRoot, { commandRunner })).toBe(paths.sidecarPath);
		expect(readFileSync(paths.sidecarPath, 'utf8')).toBe('real helper');
	});
});
