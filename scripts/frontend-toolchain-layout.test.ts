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

		const ciYml = readFileSync(path.join(repoRoot, '.github/workflows/ci.yml'), 'utf8');
		expect(ciYml).toContain('scripts/locked-bun-version.sh');
		expect(ciYml).not.toMatch(/bun-version:\s*\d/);

		const setupScript = readFileSync(
			path.join(repoRoot, 'scripts/setup-codex-agent-env.sh'),
			'utf8',
		);
		expect(setupScript).toContain('scripts/locked-bun-version.sh');
		expect(setupScript).not.toMatch(/required_bun_version="\d/);
		expect(setupScript).toContain(`/releases/download/bun-v\${required_bun_version}`);
		expect(setupScript).toContain('error: need Bun');
	});

	// Dependabot's Bun updater rejects lockfiles newer than v1; a lockfile
	// regenerated from scratch by Bun 1.4+ is v2 and silently stops Bun updates.
	it('keeps bun.lock at lockfileVersion 1 so Dependabot can update it', () => {
		const lock = readFileSync(path.join(repoRoot, 'bun.lock'), 'utf8');
		expect(lock).toMatch(/^\{\s*"lockfileVersion": 1,/);
	});
});
