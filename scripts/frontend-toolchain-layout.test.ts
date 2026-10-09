import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

type PackageManifest = {
	packageManager?: string;
	devDependencies: Record<string, string>;
};

function readPackageManifest(): PackageManifest {
	return JSON.parse(readFileSync(path.join(repoRoot, 'package.json'), 'utf8')) as PackageManifest;
}

describe('frontend toolchain layout', () => {
	it('takes the Bun toolchain version only from bun.lock', () => {
		const pkg = readPackageManifest();
		expect(pkg.packageManager).toBeUndefined();
		expect(pkg.devDependencies.bun).toMatch(/^\^\d/);
		const locked = execFileSync('bash', ['scripts/locked-bun-version.sh'], {
			cwd: repoRoot,
			encoding: 'utf8',
		}).trim();
		expect(locked).toMatch(/^\d+\.\d+\.\d+$/);
	});

	it('pins an exact Node version that setup.sh can download', () => {
		const version = readFileSync(path.join(repoRoot, '.node-version'), 'utf8').trim();
		expect(version).toMatch(/^\d+\.\d+\.\d+$/);
	});

	// Dependabot's Bun updater rejects lockfiles newer than v1; a lockfile
	// regenerated from scratch by Bun 1.4+ is v2 and silently stops Bun updates.
	it('keeps bun.lock at lockfileVersion 1 so Dependabot can update it', () => {
		const lock = readFileSync(path.join(repoRoot, 'bun.lock'), 'utf8');
		expect(lock).toMatch(/^\{\s*"lockfileVersion": 1,/);
	});
});
