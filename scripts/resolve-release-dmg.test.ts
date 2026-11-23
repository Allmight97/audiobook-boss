import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

import {
	releaseAssetFileName,
	resolveReleaseDmgArtifact,
	stageReleaseDmg,
} from './resolve-release-dmg';

function createFixture(): string {
	const repoRoot = mkdtempSync(path.join(os.tmpdir(), 'abb-release-dmg-'));
	mkdirSync(path.join(repoRoot, 'src-tauri'), { recursive: true });
	writeFileSync(
		path.join(repoRoot, 'src-tauri', 'tauri.conf.json'),
		JSON.stringify({ productName: 'AudioBook Boss' }, null, 2),
	);
	return repoRoot;
}

describe('resolveReleaseDmgArtifact', () => {
	it('resolves the single version-matching dmg artifact', () => {
		const repoRoot = createFixture();

		try {
			const bundleDir = path.join(repoRoot, 'target', 'release', 'bundle', 'dmg');
			mkdirSync(bundleDir, { recursive: true });
			const dmgPath = path.join(bundleDir, 'AudioBook Boss_1.0.5_aarch64.dmg');
			writeFileSync(dmgPath, '');

			expect(resolveReleaseDmgArtifact(repoRoot, '1.0.5')).toBe(dmgPath);
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
		}
	});

	it('fails when multiple dmg artifacts match the release version', () => {
		const repoRoot = createFixture();

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

describe('release asset name', () => {
	it('stages the DMG under the dotted asset name and records its SHA-256', () => {
		const repoRoot = createFixture();
		const stageDir = mkdtempSync(path.join(os.tmpdir(), 'abb-release-stage-'));
		try {
			const bundleDir = path.join(repoRoot, 'target', 'release', 'bundle', 'dmg');
			mkdirSync(bundleDir, { recursive: true });
			const built = path.join(bundleDir, 'AudioBook Boss_1.0.5_aarch64.dmg');
			writeFileSync(built, 'dmg-bytes');
			const asset = 'AudioBook.Boss_1.0.5_aarch64.dmg';
			expect(releaseAssetFileName(built)).toBe(asset);

			const staged = stageReleaseDmg(built, stageDir);
			expect(staged.asset).toBe(asset);
			expect(readFileSync(staged.dmg, 'utf8')).toBe('dmg-bytes');
			expect(staged.sha256).toBe(createHash('sha256').update('dmg-bytes').digest('hex'));
			expect(readFileSync(staged.checksum, 'utf8')).toBe(`${staged.sha256}  ${asset}\n`);
		} finally {
			rmSync(repoRoot, { force: true, recursive: true });
			rmSync(stageDir, { force: true, recursive: true });
		}
	});
});
