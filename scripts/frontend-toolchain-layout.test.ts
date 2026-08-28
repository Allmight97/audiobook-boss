import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

type PackageManifest = {
	packageManager: string;
	scripts: Record<string, string>;
	devDependencies: Record<string, string>;
};

function readPackageManifest(): PackageManifest {
	return JSON.parse(readFileSync(path.join(repoRoot, 'package.json'), 'utf8')) as PackageManifest;
}

function bunVersionFromPackageManager(packageManager: string): string {
	const match = /^bun@(.+)$/.exec(packageManager);
	if (!match?.[1]) {
		throw new Error(`packageManager is not a bun pin: ${packageManager}`);
	}
	return match[1];
}

function collectSourceFiles(root: string): string[] {
	const files: string[] = [];
	const skipDirs = new Set(['node_modules', 'dist', '.git', '.svelte-check', 'coverage']);

	const walk = (dir: string): void => {
		for (const entry of readdirSync(dir, { withFileTypes: true })) {
			if (entry.isDirectory()) {
				if (skipDirs.has(entry.name)) {
					continue;
				}
				walk(path.join(dir, entry.name));
				continue;
			}
			if (/\.(?:[cm]?tsx?|mjs|cjs|svelte)$/.test(entry.name)) {
				files.push(path.join(dir, entry.name));
			}
		}
	};

	walk(root);
	return files;
}

function importsTypescriptPackage(source: string): boolean {
	return /\b(?:from\s+|import\s*\(\s*|require\s*\(\s*|import\s+)['"]typescript['"]/.test(
		source,
	);
}

function typescriptImportHits(): string[] {
	const self = path.normalize(fileURLToPath(import.meta.url));
	const hits: string[] = [];
	for (const root of ['src', 'scripts']) {
		for (const file of collectSourceFiles(path.join(repoRoot, root))) {
			if (path.normalize(file) === self) {
				continue;
			}
			if (importsTypescriptPackage(readFileSync(file, 'utf8'))) {
				hits.push(path.relative(repoRoot, file));
			}
		}
	}
	return hits;
}

describe('frontend toolchain layout', () => {
	it('keeps TypeScript 7 in @typescript/native and the 6.x require() shim in the typescript slot', () => {
		const pkg = readPackageManifest();
		expect(pkg.devDependencies['@typescript/native']).toMatch(/^npm:typescript@7\./);
		expect(pkg.devDependencies.typescript).toMatch(/^npm:@typescript\/typescript6@/);
		expect(pkg.scripts['check:svelte']).toContain('--tsgo');
	});

	it('keeps CI and Codex Bun pins locked to package.json packageManager', () => {
		const bunVersion = bunVersionFromPackageManager(readPackageManifest().packageManager);
		const ciYml = readFileSync(path.join(repoRoot, '.github/workflows/ci.yml'), 'utf8');
		const setupScript = readFileSync(
			path.join(repoRoot, 'scripts/setup-codex-agent-env.sh'),
			'utf8',
		);
		expect(ciYml).toContain(`bun-version: ${bunVersion}`);
		expect(setupScript).toContain(`required_bun_version="${bunVersion}"`);
		expect(setupScript).toContain(`BUN_VERSION="\${required_bun_version}"`);
		expect(setupScript).toContain('error: need Bun');
	});

	it('does not import the typescript package from ABB src/ or scripts/', () => {
		expect(typescriptImportHits()).toEqual([]);
	});

	it('records typescript specifiers from multiline named imports', () => {
		expect(
			importsTypescriptPackage("import {\n\tcreateSourceFile,\n} from 'typescript';\n"),
		).toBe(true);
		expect(importsTypescriptPackage("import { createSourceFile } from './typescript';\n")).toBe(
			false,
		);
	});
});
