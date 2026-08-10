import { existsSync, mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import type { spawnSync } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import { afterAll, describe, expect, it } from 'vitest';

import {
	addAaxcleanHelperConfigArg,
	assertSupportedMacOsHost,
	buildTauriApp,
	bundledFfmpegFeatureForBuild,
	ensureFeatureArg,
	findForbiddenLinkedLibraries,
	findUnsupportedMacOsArchitectures,
	installLocalApplicationBundle,
	pruneLocalInstallArtifacts,
	resolveRequestedBundles,
	resolveMacOsBundlePaths,
	verifyDmgBundle,
} from './build-app';

const tempRoots: string[] = [];

function createRepoFixture(): { applicationsDir: string; repoRoot: string } {
	const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-build-app-'));
	const applicationsDir = path.join(repoRoot, 'Applications');
	tempRoots.push(repoRoot);

	writeFileSync(path.join(repoRoot, 'package.json'), JSON.stringify({ name: 'audiobook-boss' }));
	mkdirSync(path.join(repoRoot, 'src-tauri'), { recursive: true });
	writeFileSync(
		path.join(repoRoot, 'src-tauri/tauri.conf.json'),
		JSON.stringify({ productName: 'AudioBook Boss' }),
	);
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
	mkdirSync(path.join(repoRoot, 'target/release/bundle/macos'), { recursive: true });
	mkdirSync(applicationsDir, { recursive: true });

	return { applicationsDir, repoRoot };
}

afterAll(() => {
	for (const tempRoot of tempRoots.splice(0, tempRoots.length)) {
		rmSync(tempRoot, { force: true, recursive: true });
	}
});

describe('resolveMacOsBundlePaths', () => {
	it('derives canonical macOS bundle paths from repo metadata', () => {
		const { applicationsDir, repoRoot } = createRepoFixture();
		const paths = resolveMacOsBundlePaths(repoRoot, applicationsDir);

		expect(paths.canonicalAppPath).toBe(
			path.join(repoRoot, 'target/release/bundle/macos/AudioBook Boss.app'),
		);
		expect(paths.executablePath).toBe(
			path.join(
				repoRoot,
				'target/release/bundle/macos/AudioBook Boss.app/Contents/MacOS/audiobook-boss',
			),
		);
		expect(paths.helperExecutablePath).toBe(
			path.join(
				repoRoot,
				'target/release/bundle/macos/AudioBook Boss.app/Contents/MacOS/abb-aaxclean-helper',
			),
		);
		expect(paths.applicationsAppPath).toBe(path.join(applicationsDir, 'AudioBook Boss.app'));
		expect(paths.dmgDir).toBe(path.join(repoRoot, 'target/release/bundle/dmg'));
	});
});

describe('bundledFfmpegFeatureForBuild', () => {
	it('uses native host tuning for source-built app bundles', () => {
		expect(bundledFfmpegFeatureForBuild(['--bundles', 'app'])).toBe('bundled-ffmpeg');
	});

	it('uses the portable target for every build that produces a DMG', () => {
		expect(bundledFfmpegFeatureForBuild(['--bundles', 'dmg'])).toBe('bundled-ffmpeg-portable');
		expect(bundledFfmpegFeatureForBuild(['--bundles', 'all'])).toBe('bundled-ffmpeg-portable');
	});
});

describe('findForbiddenLinkedLibraries', () => {
	it('detects Homebrew-linked dylibs in otool output', () => {
		const output = [
			'/tmp/AudioBook Boss.app/Contents/MacOS/audiobook-boss:',
			'\t/System/Library/Frameworks/AppKit.framework/Versions/C/AppKit',
			'\t/opt/homebrew/opt/ffmpeg/lib/libavcodec.62.dylib',
			'\t/usr/local/opt/libx11/lib/libX11.6.dylib',
		].join('\n');

		expect(findForbiddenLinkedLibraries(output)).toEqual([
			'/opt/homebrew/opt/ffmpeg/lib/libavcodec.62.dylib',
			'/usr/local/opt/libx11/lib/libX11.6.dylib',
		]);
	});
});

describe('installLocalApplicationBundle', () => {
	it('copies, signs, verifies, registers, and imports the Applications bundle', () => {
		const { applicationsDir, repoRoot } = createRepoFixture();
		const paths = resolveMacOsBundlePaths(repoRoot, applicationsDir);
		mkdirSync(paths.canonicalAppPath, { recursive: true });
		mkdirSync(paths.applicationsAppPath, { recursive: true });

		const calls: Array<{ args: string[]; command: string }> = [];
		const commandRunner = ((command: string, args: string[]) => {
			calls.push({ command, args });
			return { status: 0 };
		}) as typeof spawnSync;

		installLocalApplicationBundle(paths, commandRunner);

		expect(existsSync(paths.applicationsAppPath)).toBe(false);
		expect(calls).toEqual([
			{ command: 'ditto', args: [paths.canonicalAppPath, paths.applicationsAppPath] },
			{
				command: 'codesign',
				args: ['--force', '--deep', '--sign', '-', paths.applicationsAppPath],
			},
			{
				command: 'codesign',
				args: ['--verify', '--deep', '--strict', '--verbose=2', paths.applicationsAppPath],
			},
			{
				command:
					'/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister',
				args: ['-f', paths.applicationsAppPath],
			},
			{ command: 'mdimport', args: [paths.applicationsAppPath] },
		]);
	});

	it('fails when a local install command fails', () => {
		const { applicationsDir, repoRoot } = createRepoFixture();
		const paths = resolveMacOsBundlePaths(repoRoot, applicationsDir);
		mkdirSync(paths.canonicalAppPath, { recursive: true });

		const commandRunner = ((command: string) => ({
			status: command === 'codesign' ? 1 : 0,
		})) as typeof spawnSync;

		expect(() => installLocalApplicationBundle(paths, commandRunner)).toThrow(
			'codesign failed with status 1',
		);
	});
});

describe('pruneLocalInstallArtifacts', () => {
	it('removes the repo-local app bundle and temporary writable dmg residue', () => {
		const { applicationsDir, repoRoot } = createRepoFixture();
		const paths = resolveMacOsBundlePaths(repoRoot, applicationsDir);
		mkdirSync(paths.canonicalAppPath, { recursive: true });
		writeFileSync(path.join(paths.bundleDir, 'rw.123.AudioBook Boss_1.0.23_aarch64.dmg'), '');
		writeFileSync(path.join(paths.bundleDir, 'AudioBook Boss_1.0.23_aarch64.dmg'), '');

		expect(pruneLocalInstallArtifacts(paths).sort()).toEqual(
			[
				paths.canonicalAppPath,
				path.join(paths.bundleDir, 'rw.123.AudioBook Boss_1.0.23_aarch64.dmg'),
			].sort(),
		);
		expect(existsSync(paths.canonicalAppPath)).toBe(false);
		expect(existsSync(path.join(paths.bundleDir, 'rw.123.AudioBook Boss_1.0.23_aarch64.dmg'))).toBe(
			false,
		);
		expect(existsSync(path.join(paths.bundleDir, 'AudioBook Boss_1.0.23_aarch64.dmg'))).toBe(true);
	});
});

describe('assertSupportedMacOsHost', () => {
	it('allows Apple Silicon macOS hosts', () => {
		expect(() => assertSupportedMacOsHost('darwin', 'arm64')).not.toThrow();
	});

	it('rejects Intel or translated macOS hosts', () => {
		expect(() => assertSupportedMacOsHost('darwin', 'x64')).toThrow(
			"AudioBook Boss supports only native Apple Silicon macOS hosts. Refusing host architecture 'x64'.",
		);
	});
});

describe('findUnsupportedMacOsArchitectures', () => {
	it('rejects non-arm64 slices in packaged binaries', () => {
		expect(findUnsupportedMacOsArchitectures('arm64')).toEqual([]);
		expect(findUnsupportedMacOsArchitectures('arm64 arm64e')).toEqual([]);
		expect(findUnsupportedMacOsArchitectures('x86_64 arm64')).toEqual(['x86_64']);
	});
});

describe('ensureFeatureArg', () => {
	it('adds bundled-ffmpeg when no features flag is present', () => {
		expect(ensureFeatureArg(['--bundles', 'app'], '--features', 'bundled-ffmpeg')).toEqual([
			'--bundles',
			'app',
			'--features',
			'bundled-ffmpeg',
		]);
	});

	it('merges bundled-ffmpeg into --features value', () => {
		expect(
			ensureFeatureArg(['--features', 'custom-protocol'], '--features', 'bundled-ffmpeg'),
		).toEqual(['--features', 'custom-protocol,bundled-ffmpeg']);
	});

	it('merges bundled-ffmpeg into --features=value', () => {
		expect(
			ensureFeatureArg(['--features=custom-protocol'], '--features', 'bundled-ffmpeg'),
		).toEqual(['--features=custom-protocol,bundled-ffmpeg']);
	});

	it('merges bundled-ffmpeg into -f value', () => {
		expect(ensureFeatureArg(['-f', 'custom-protocol'], '--features', 'bundled-ffmpeg')).toEqual([
			'-f',
			'custom-protocol,bundled-ffmpeg',
		]);
	});

	it('does not duplicate bundled-ffmpeg when already present', () => {
		expect(
			ensureFeatureArg(
				['--features', 'custom-protocol,bundled-ffmpeg'],
				'--features',
				'bundled-ffmpeg',
			),
		).toEqual(['--features', 'custom-protocol,bundled-ffmpeg']);
	});

	it('ignores arguments after -- boundary', () => {
		expect(
			ensureFeatureArg(
				['--features', 'custom-protocol', '--', '--features', 'runner-only'],
				'--features',
				'bundled-ffmpeg',
			),
		).toEqual(['--features', 'custom-protocol,bundled-ffmpeg', '--', '--features', 'runner-only']);
	});

	it('fills malformed repeated features flags predictably', () => {
		expect(
			ensureFeatureArg(
				['--features', '--bundles', 'app', '--features', 'custom-protocol'],
				'--features',
				'bundled-ffmpeg',
			),
		).toEqual([
			'--features',
			'bundled-ffmpeg',
			'--bundles',
			'app',
			'--features',
			'custom-protocol,bundled-ffmpeg',
		]);
	});
});

describe('addAaxcleanHelperConfigArg', () => {
	it('injects the AAXClean helper sidecar as a Tauri CLI config overlay', () => {
		const args = addAaxcleanHelperConfigArg(['--bundles', 'app']);
		const configIndex = args.indexOf('--config');

		expect(args.slice(0, configIndex)).toEqual(['--bundles', 'app']);
		expect(JSON.parse(args[configIndex + 1] ?? '{}')).toEqual({
			bundle: {
				externalBin: ['binaries/abb-aaxclean-helper'],
			},
		});
	});

	it('inserts the sidecar config before arguments after the boundary', () => {
		const args = addAaxcleanHelperConfigArg(['--bundles', 'app', '--', '--runner-only']);
		const boundaryIndex = args.indexOf('--');
		const configIndex = args.indexOf('--config');

		expect(configIndex).toBeGreaterThan(-1);
		expect(configIndex).toBeLessThan(boundaryIndex);
		expect(args.slice(boundaryIndex)).toEqual(['--', '--runner-only']);
	});
});

describe('buildTauriApp', () => {
	it('forces noninteractive Finder-free DMG packaging when requested', () => {
		const { repoRoot } = createRepoFixture();
		const calls: Array<{
			args: string[];
			command: string;
			env?: NodeJS.ProcessEnv;
			stdio?: unknown;
		}> = [];
		const commandRunner = ((
			command: string,
			args: string[],
			options?: { env?: NodeJS.ProcessEnv; stdio?: unknown },
		) => {
			calls.push({ command, args, env: options?.env, stdio: options?.stdio });
			if (args[0] === 'publish') {
				const outputDir = args[args.indexOf('-o') + 1] as string;
				mkdirSync(outputDir, { recursive: true });
				writeFileSync(path.join(outputDir, 'abb-aaxclean-helper'), 'helper');
			}
			return { status: 0 };
		}) as typeof spawnSync;

		buildTauriApp(repoRoot, ['--bundles', 'dmg'], {
			commandRunner,
			nonInteractiveDmg: true,
		});

		expect(calls).toHaveLength(2);
		expect(calls[1]?.command).toBe('bun');
		expect(calls[1]?.args.slice(0, 8)).toEqual([
			'run',
			'tauri',
			'build',
			'--bundles',
			'dmg',
			'--features',
			'bundled-ffmpeg-portable',
			'--config',
		]);
		expect(JSON.parse(calls[1]?.args[8] ?? '{}')).toEqual({
			bundle: { externalBin: ['binaries/abb-aaxclean-helper'] },
		});
		expect(calls[1]?.stdio).toBe('inherit');
		expect(calls[1]?.env?.CI).toBe('true');
	});

	it('leaves CI value untouched for non-DMG app builds', () => {
		const { repoRoot } = createRepoFixture();
		const calls: Array<{ env?: NodeJS.ProcessEnv }> = [];
		const commandRunner = ((
			command: string,
			args: string[],
			options?: { env?: NodeJS.ProcessEnv },
		) => {
			calls.push({ env: options?.env });
			if (args[0] === 'publish') {
				const outputDir = args[args.indexOf('-o') + 1] as string;
				mkdirSync(outputDir, { recursive: true });
				writeFileSync(path.join(outputDir, 'abb-aaxclean-helper'), 'helper');
			}
			void command;
			return { status: 0 };
		}) as typeof spawnSync;

		buildTauriApp(repoRoot, ['--bundles', 'app'], {
			commandRunner,
			nonInteractiveDmg: false,
		});

		expect(calls[1]?.env?.CI).toBe(process.env.CI);
		expect(calls[1]?.env).toBe(process.env);
	});
});

describe('resolveRequestedBundles', () => {
	it('defaults to app and dmg when no bundle selection is passed', () => {
		expect(resolveRequestedBundles([])).toEqual(new Set(['app', 'dmg']));
	});

	it('parses explicit app and dmg bundle selections', () => {
		expect(resolveRequestedBundles(['--bundles', 'dmg'])).toEqual(new Set(['dmg']));
		expect(resolveRequestedBundles(['--bundles=app,dmg'])).toEqual(new Set(['app', 'dmg']));
		expect(resolveRequestedBundles(['-b', 'app'])).toEqual(new Set(['app']));
	});

	it('treats explicit all as app and dmg', () => {
		expect(resolveRequestedBundles(['--bundles', 'all'])).toEqual(new Set(['app', 'dmg']));
	});
});

describe('verifyDmgBundle', () => {
	it('accepts dmg artifacts without requiring a surviving app bundle', () => {
		const { applicationsDir, repoRoot } = createRepoFixture();
		const paths = resolveMacOsBundlePaths(repoRoot, applicationsDir);
		mkdirSync(paths.dmgDir, { recursive: true });
		writeFileSync(path.join(paths.dmgDir, 'AudioBook Boss_1.0.12_aarch64.dmg'), '');

		expect(() => verifyDmgBundle(paths)).not.toThrow();
	});

	it('rejects missing dmg artifacts', () => {
		const { applicationsDir, repoRoot } = createRepoFixture();
		const paths = resolveMacOsBundlePaths(repoRoot, applicationsDir);
		mkdirSync(paths.dmgDir, { recursive: true });

		expect(() => verifyDmgBundle(paths)).toThrow(
			`Expected at least one dmg artifact under ${paths.dmgDir}`,
		);
	});
});
