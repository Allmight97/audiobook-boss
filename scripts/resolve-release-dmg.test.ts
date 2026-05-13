import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'bun:test';

import { resolveReleaseDmgArtifact } from './resolve-release-dmg';

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
