import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

type PackageManifest = {
	packageManager: string;
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
	const skipDirs = new Set(['node_modules', 'dist', '.git', 'coverage']);

	const walk = (dir: string): void => {
		for (const entry of readdirSync(dir, { withFileTypes: true })) {
			if (entry.isDirectory()) {
				if (skipDirs.has(entry.name)) {
					continue;
				}
				walk(path.join(dir, entry.name));
				continue;
			}
			if (/\.(?:[cm]?tsx?|mjs|cjs)$/.test(entry.name)) {
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
	it('keeps CI and Codex Bun pins locked to package.json packageManager', () => {
		const bunVersion = bunVersionFromPackageManager(readPackageManifest().packageManager);
		const ciYml = readFileSync(path.join(repoRoot, '.github/workflows/ci.yml'), 'utf8');
		const setupScript = readFileSync(
			path.join(repoRoot, 'scripts/setup-codex-agent-env.sh'),
			'utf8',
		);
		expect(ciYml).toContain(`bun-version: ${bunVersion}`);
		expect(setupScript).toContain(`required_bun_version="${bunVersion}"`);
		expect(setupScript).toContain(`/releases/download/bun-v\${required_bun_version}`);
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

	it('does not import Tailwind packages from ABB src/ or scripts/', () => {
		const hits = packageImportHits((source) =>
			/\b(?:from\s+|import\s*\(\s*|require\s*\(\s*|import\s+)['"](?:tailwindcss|@tailwindcss\/vite)(?:\/[^'"]*)?['"]/.test(
				source,
			),
		);
		expect(hits).toEqual([]);
	});

	it('keeps Tailwind out of package.json', () => {
		const pkg = readPackageManifest();
		expect(pkg.devDependencies.tailwindcss).toBeUndefined();
		expect(pkg.devDependencies['@tailwindcss/vite']).toBeUndefined();
	});

	it('does not import foundation internals from outside foundation', () => {
		const foundationRoot = path.normalize(path.join(repoRoot, 'src/ui/foundation'));
		const hits: string[] = [];
		for (const file of collectSourceFiles(path.join(repoRoot, 'src'))) {
			const normalized = path.normalize(file);
			if (normalized.startsWith(`${foundationRoot}${path.sep}`)) continue;
			const source = readFileSync(file, 'utf8');
			if (
				/\b(?:from\s+|import\s*\(\s*)['"][^'"]*ui\/foundation\/internal(?:\/[^'"]*)?['"]/.test(
					source,
				)
			) {
				hits.push(path.relative(repoRoot, file));
			}
		}
		expect(hits).toEqual([]);
	});
});
