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
	return /\b(?:from\s+|import\s*\(\s*|require\s*\(\s*|import\s+)['"]typescript['"]/.test(source);
}

function importsEffectPackageRoot(source: string): boolean {
	return /\b(?:from\s+|import\s*\(\s*|require\s*\(\s*|import\s+)['"]effect['"]/.test(source);
}

function importsEffectPackage(source: string): boolean {
	return /\b(?:from\s+|import\s*\(\s*|require\s*\(\s*|import\s+)['"]effect(?:\/[^'"]*)?['"]/.test(
		source,
	);
}

function importsEffectReactivity(source: string): boolean {
	return /\b(?:from\s+|import\s*\(\s*|require\s*\(\s*|import\s+)['"]effect\/unstable\/reactivity(?:\/[^'"]*)?['"]/.test(
		source,
	);
}

function importsAtomSolid(source: string): boolean {
	return /\b(?:from\s+|import\s*\(\s*|require\s*\(\s*|import\s+)['"]@effect\/atom-solid(?:\/[^'"]*)?['"]/.test(
		source,
	);
}

function packageImportHits(
	matches: (source: string) => boolean,
	skip: ReadonlySet<string> = new Set(),
): string[] {
	const self = path.normalize(fileURLToPath(import.meta.url));
	const hits: string[] = [];
	for (const root of ['src', 'scripts']) {
		for (const file of collectSourceFiles(path.join(repoRoot, root))) {
			const normalized = path.normalize(file);
			if (normalized === self || skip.has(normalized)) {
				continue;
			}
			if (matches(readFileSync(file, 'utf8'))) {
				hits.push(path.relative(repoRoot, file));
			}
		}
	}
	return hits;
}

describe('frontend toolchain layout', () => {
	it('keeps TypeScript 7 in both @typescript/native and the typescript slot', () => {
		const pkg = readPackageManifest();
		expect(pkg.devDependencies['@typescript/native']).toMatch(/^npm:typescript@7\./);
		expect(pkg.devDependencies.typescript).toMatch(/^npm:typescript@7\./);
		expect(pkg.scripts['check:svelte']).toBeUndefined();
		expect(pkg.devDependencies.svelte).toBeUndefined();
		expect(pkg.devDependencies['@testing-library/svelte']).toBeUndefined();
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
		expect(packageImportHits(importsTypescriptPackage)).toEqual([]);
	});

	it('records typescript specifiers from multiline named imports', () => {
		expect(importsTypescriptPackage("import {\n\tcreateSourceFile,\n} from 'typescript';\n")).toBe(
			true,
		);
		expect(importsTypescriptPackage("import { createSourceFile } from './typescript';\n")).toBe(
			false,
		);
	});

	it('does not keep a TypeScript 6 compiler in bun.lock', () => {
		const lock = readFileSync(path.join(repoRoot, 'bun.lock'), 'utf8');
		expect(lock).not.toContain('"@typescript/old": ["typescript@6.');
		expect(lock).not.toMatch(/"@typescript\/typescript6@/);
	});

	it('has no leftover Svelte sources under src/', () => {
		const svelteSources = collectSourceFiles(path.join(repoRoot, 'src')).filter((file) =>
			/\.svelte(?:\.ts)?$/.test(file),
		);
		expect(svelteSources).toEqual([]);
	});

	it('lets only appEffect.ts import the effect package root', () => {
		const allowed = path.normalize(path.join(repoRoot, 'src/lib/effect/appEffect.ts'));
		expect(packageImportHits(importsEffectPackageRoot, new Set([allowed]))).toEqual([]);
	});

	it('lets only the ABB workflow and reactivity seams import the effect package family', () => {
		const allowed = new Set([
			path.normalize(path.join(repoRoot, 'src/lib/effect/appEffect.ts')),
			path.normalize(path.join(repoRoot, 'src/app/runtime/reactivity.ts')),
		]);
		expect(packageImportHits(importsEffectPackage, allowed)).toEqual([]);
	});

	it('lets only the runtime reactivity seam import effect/unstable/reactivity', () => {
		const allowed = path.normalize(path.join(repoRoot, 'src/app/runtime/reactivity.ts'));
		expect(packageImportHits(importsEffectReactivity, new Set([allowed]))).toEqual([]);
	});

	it('lets only the Solid integration seam import @effect/atom-solid', () => {
		const allowed = path.normalize(path.join(repoRoot, 'src/app/runtime/solid.ts'));
		expect(packageImportHits(importsAtomSolid, new Set([allowed]))).toEqual([]);
	});

	it('treats Effect package root and subpath specifiers as the same family', () => {
		expect(importsEffectPackage("import { Effect } from 'effect'")).toBe(true);
		expect(importsEffectPackage("import { Effect } from 'effect/Effect'")).toBe(true);
		expect(importsEffectPackageRoot("import { Effect } from 'effect'")).toBe(true);
		expect(importsEffectPackageRoot("import { Atom } from 'effect/unstable/reactivity'")).toBe(
			false,
		);
		expect(importsEffectPackage("import { something } from './effect'")).toBe(false);
	});

	it('records effect specifiers from multiline named imports', () => {
		expect(importsEffectPackage("import {\n\tEffect,\n} from 'effect';\n")).toBe(true);
		expect(importsEffectPackage("import { Effect } from './effect';\n")).toBe(false);
	});
});
