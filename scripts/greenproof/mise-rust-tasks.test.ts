import { readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'bun:test';

const repoRoot = path.resolve(import.meta.dir, '../..');

const LEGACY_PROOF_PATTERNS = [
	'scripts/proof.sh',
	'scripts/proof/',
	'scripts/proof/runner.ts',
	'events.ndjson',
	'.proof/runs/',
	'bun scripts/proof',
] as const;

function collectTaskSources(): string[] {
	const sources = [readFileSync(path.join(repoRoot, 'mise.toml'), 'utf8')];
	const tasksDir = path.join(repoRoot, 'mise/tasks');
	try {
		for (const entry of readdirSync(tasksDir)) {
			const full = path.join(tasksDir, entry);
			if (statSync(full).isFile()) {
				sources.push(readFileSync(full, 'utf8'));
			}
		}
	} catch {
		// optional tasks dir
	}
	return sources;
}

function collectDocSurfaces(): string[] {
	const surfaces: string[] = [
		path.join(repoRoot, 'AGENTS.md'),
		path.join(repoRoot, 'README.md'),
		path.join(repoRoot, 'package.json'),
		path.join(repoRoot, 'docs/greenproof.md'),
		path.join(repoRoot, 'docs/api-map.md'),
		path.join(repoRoot, 'docs/ubiquitous-language.md'),
		path.join(repoRoot, 'docs/fallbacks.md'),
		path.join(repoRoot, 'docs/system-map.md'),
	];

	const docsDir = path.join(repoRoot, 'docs');
	for (const entry of readdirSync(docsDir)) {
		const full = path.join(docsDir, entry);
		if (statSync(full).isFile() && entry.endsWith('.md') && entry !== 'CHANGELOG.md') {
			surfaces.push(full);
		}
		if (statSync(full).isDirectory() && entry === 'specs') {
			for (const spec of readdirSync(full)) {
				if (spec.endsWith('.md')) {
					surfaces.push(path.join(full, spec));
				}
			}
		}
	}

	const agentsRoots = [path.join(repoRoot, 'src'), path.join(repoRoot, 'src-tauri')];
	for (const root of agentsRoots) {
		const walk = (dir: string) => {
			for (const entry of readdirSync(dir)) {
				const full = path.join(dir, entry);
				const st = statSync(full);
				if (st.isDirectory()) {
					walk(full);
					continue;
				}
				if (entry === 'AGENTS.md') {
					surfaces.push(full);
				}
			}
		};
		walk(root);
	}

	const skillsDir = path.join(repoRoot, '.agents/skills');
	for (const skill of readdirSync(skillsDir)) {
		const skillRoot = path.join(skillsDir, skill);
		if (!statSync(skillRoot).isDirectory()) {
			continue;
		}
		const skillMd = path.join(skillRoot, 'SKILL.md');
		if (statSync(skillMd).isFile()) {
			surfaces.push(skillMd);
		}
		const refsDir = path.join(skillRoot, 'references');
		try {
			for (const ref of readdirSync(refsDir)) {
				if (ref.endsWith('.md')) {
					surfaces.push(path.join(refsDir, ref));
				}
			}
		} catch {
			// no references dir
		}
	}

	return surfaces;
}

function cargoTestLines(source: string): string[] {
	return source
		.split('\n')
		.filter((line) => line.includes('cargo test -p audiobook-boss'))
		.map((line) => line.trim());
}

function legacyHits(content: string, filePath: string): string[] {
	const hits: string[] = [];
	for (const pattern of LEGACY_PROOF_PATTERNS) {
		if (content.includes(pattern)) {
			hits.push(`${filePath}: ${pattern}`);
		}
	}
	return hits;
}

describe('GreenProof mise Rust task guard', () => {
	it('requires --lib or --test for audiobook-boss cargo test commands', () => {
		const violations: string[] = [];
		for (const line of collectTaskSources().flatMap(cargoTestLines)) {
			if (line.includes('--lib') || line.includes('--test')) {
				continue;
			}
			violations.push(line);
		}
		expect(violations).toEqual([]);
	});
});

describe('GreenProof doc surface guard', () => {
	it('does not reference legacy proof infrastructure', () => {
		const violations = collectDocSurfaces().flatMap((filePath) =>
			legacyHits(readFileSync(filePath, 'utf8'), path.relative(repoRoot, filePath)),
		);
		expect(violations).toEqual([]);
	});

	it('points agents at greenproof.md or mise tasks for proof routing', () => {
		const agents = readFileSync(path.join(repoRoot, 'AGENTS.md'), 'utf8');
		expect(agents).toContain('docs/greenproof.md');
		expect(agents).toContain('mise run proof');
	});
});
