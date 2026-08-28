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

function blankComments(source: string): string {
	let output = '';
	let index = 0;
	let quote: "'" | '"' | '`' | null = null;
	while (index < source.length) {
		const character = source[index] ?? '';
		const next = source[index + 1] ?? '';
		if (quote) {
			output += character;
			if (character === '\\' && quote !== '`') {
				output += next;
				index += 2;
				continue;
			}
			if (character === quote) {
				quote = null;
			}
			index += 1;
			continue;
		}
		if (character === '/' && next === '/') {
			const end = source.indexOf('\n', index);
			const length = (end === -1 ? source.length : end) - index;
			output += ' '.repeat(length);
			index += length;
			continue;
		}
		if (character === '/' && next === '*') {
			const end = source.indexOf('*/', index + 2);
			const close = end === -1 ? source.length : end + 2;
			output += source.slice(index, close).replace(/[^\n]/g, ' ');
			index = close;
			continue;
		}
		if (character === "'" || character === '"' || character === '`') {
			quote = character;
		}
		output += character;
		index += 1;
	}
	return output;
}

function moduleSpecifiers(source: string): string[] {
	const code = blankComments(source);
	const specifiers: string[] = [];
	const pattern =
		/\b(?:import\s*\(\s*|require\s*\(\s*|(?:import|export)(?:\s+type)?\s+(?:[^'"\n]*?\sfrom\s*)?)['"]([^'"]+)['"]/g;
	for (const match of code.matchAll(pattern)) {
		if (match[1]) {
			specifiers.push(match[1]);
		}
	}
	return specifiers;
}

function typescriptImportHits(): string[] {
	const hits: string[] = [];
	for (const root of ['src', 'scripts']) {
		for (const file of collectSourceFiles(path.join(repoRoot, root))) {
			const specifiers = moduleSpecifiers(readFileSync(file, 'utf8'));
			if (specifiers.includes('typescript')) {
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
});
