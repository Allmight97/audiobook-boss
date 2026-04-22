import {
	lstatSync,
	mkdtempSync,
	mkdirSync,
	readlinkSync,
	rmSync,
	symlinkSync,
	unlinkSync,
	writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterAll, describe, expect, it } from 'bun:test';

import {
	assertSupportedMacOsHost,
	ensureFeatureArg,
	findForbiddenLinkedLibraries,
	findUnsupportedMacOsArchitectures,
	refreshApplicationsLink,
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
		expect(paths.applicationsLinkPath).toBe(path.join(applicationsDir, 'AudioBook Boss.app'));
		expect(paths.dmgDir).toBe(path.join(repoRoot, 'target/release/bundle/dmg'));
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

describe('resolveRequestedBundles', () => {
	it('defaults to app when no bundle selection is passed', () => {
		expect([...resolveRequestedBundles([])]).toEqual(['app']);
	});

	it('parses explicit app and dmg bundle selections', () => {
		expect(resolveRequestedBundles(['--bundles', 'dmg'])).toEqual(new Set(['dmg']));
		expect(resolveRequestedBundles(['--bundles=app,dmg'])).toEqual(new Set(['app', 'dmg']));
		expect(resolveRequestedBundles(['-b', 'app'])).toEqual(new Set(['app']));
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

describe('refreshApplicationsLink', () => {
	it('creates a canonical /Applications symlink for the built bundle', () => {
		const { applicationsDir, repoRoot } = createRepoFixture();
		const paths = resolveMacOsBundlePaths(repoRoot, applicationsDir);
		mkdirSync(paths.canonicalAppPath, { recursive: true });

		expect(refreshApplicationsLink(paths)).toBe('created');
		expect(lstatSync(paths.applicationsLinkPath).isSymbolicLink()).toBe(true);
		expect(readlinkSync(paths.applicationsLinkPath)).toBe(paths.canonicalAppPath);
	});

	it('updates an outdated symlink target', () => {
		const { applicationsDir, repoRoot } = createRepoFixture();
		const paths = resolveMacOsBundlePaths(repoRoot, applicationsDir);
		mkdirSync(paths.canonicalAppPath, { recursive: true });
		symlinkSync(
			path.join(repoRoot, 'target/release/bundle/macos/Old.app'),
			paths.applicationsLinkPath,
			'dir',
		);

		expect(refreshApplicationsLink(paths)).toBe('updated');
		expect(readlinkSync(paths.applicationsLinkPath)).toBe(paths.canonicalAppPath);
	});

	it('skips when the symlink already points to canonical app', () => {
		const { applicationsDir, repoRoot } = createRepoFixture();
		const paths = resolveMacOsBundlePaths(repoRoot, applicationsDir);
		mkdirSync(paths.canonicalAppPath, { recursive: true });
		symlinkSync(paths.canonicalAppPath, paths.applicationsLinkPath, 'dir');

		expect(refreshApplicationsLink(paths)).toBe('skipped');
		expect(readlinkSync(paths.applicationsLinkPath)).toBe(paths.canonicalAppPath);
	});

	it('skips when existing /Applications path is not a symlink', () => {
		const { applicationsDir, repoRoot } = createRepoFixture();
		const paths = resolveMacOsBundlePaths(repoRoot, applicationsDir);
		mkdirSync(paths.canonicalAppPath, { recursive: true });
		mkdirSync(paths.applicationsLinkPath, { recursive: true });

		expect(refreshApplicationsLink(paths)).toBe('skipped');
		expect(lstatSync(paths.applicationsLinkPath).isSymbolicLink()).toBe(false);
	});

	it('returns skipped when lstat is permission denied', () => {
		const { applicationsDir, repoRoot } = createRepoFixture();
		const paths = resolveMacOsBundlePaths(repoRoot, applicationsDir);
		mkdirSync(paths.canonicalAppPath, { recursive: true });

		const outcome = refreshApplicationsLink(paths, {
			lstatSync: () => {
				const error = new Error('permission denied') as NodeJS.ErrnoException;
				error.code = 'EACCES';
				throw error;
			},
			mkdirSync,
			readlinkSync,
			symlinkSync,
			unlinkSync,
		});
		expect(outcome).toBe('skipped');
	});

	it('returns skipped when replacing symlink is permission denied', () => {
		const { applicationsDir, repoRoot } = createRepoFixture();
		const paths = resolveMacOsBundlePaths(repoRoot, applicationsDir);
		mkdirSync(paths.canonicalAppPath, { recursive: true });
		symlinkSync(
			path.join(repoRoot, 'target/release/bundle/macos/Old.app'),
			paths.applicationsLinkPath,
			'dir',
		);

		const outcome = refreshApplicationsLink(paths, {
			lstatSync,
			mkdirSync,
			readlinkSync,
			symlinkSync,
			unlinkSync: () => {
				const error = new Error('operation not permitted') as NodeJS.ErrnoException;
				error.code = 'EPERM';
				throw error;
			},
		});
		expect(outcome).toBe('skipped');
		expect(readlinkSync(paths.applicationsLinkPath)).toBe(
			path.join(repoRoot, 'target/release/bundle/macos/Old.app'),
		);
	});
});
