import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { parseDocument } from 'yaml';

import { laneMatcher, laneMap, requiredLanes } from './verification';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const mapPath = path.join(repoRoot, 'scripts/lane-paths.yml');

function patternsIn(value: unknown): string[] {
	if (Array.isArray(value)) return value.flat(Number.POSITIVE_INFINITY) as string[];
	if (value !== null && typeof value === 'object') return Object.keys(value);
	return [];
}

function unmappedPaths(mapYaml: string, files: string[]): string[] {
	const filters = parseDocument(mapYaml).toJS() as Record<string, unknown>;
	const patterns = Object.values(filters)
		.flatMap((value) => patternsIn(value))
		.filter((pattern) => !pattern.startsWith('!'));
	const isMatch = laneMatcher(patterns);
	return files.filter((file) => !isMatch(file));
}

function lanesFor(mapYaml: string, lane: string): (file: string) => boolean {
	const filters = parseDocument(mapYaml).toJS() as Record<string, unknown>;
	return laneMatcher(patternsIn(filters[lane]));
}

function noLaneEntriesWithoutReason(mapYaml: string): string[] {
	const value = (parseDocument(mapYaml).toJS() as Record<string, unknown> | null)?.['no-lane'];
	if (value === null || typeof value !== 'object' || Array.isArray(value)) return ['no-lane'];
	return Object.entries(value)
		.filter(([, reason]) => typeof reason !== 'string' || reason.trim() === '')
		.map(([pattern]) => pattern);
}

describe('scripts/lane-paths.yml', () => {
	const mapYaml = readFileSync(mapPath, 'utf8');

	it('maps every tracked file to a lane or a no-lane entry', () => {
		const tracked = execFileSync('git', ['ls-files'], { cwd: repoRoot, encoding: 'utf8' })
			.split('\n')
			.filter(Boolean);
		expect(
			unmappedPaths(mapYaml, tracked),
			'Add each path to a lane in scripts/lane-paths.yml, or to no-lane with a reason',
		).toEqual([]);
	});

	it('runs real-media tests for a change to any crate, since they drive the whole engine', () => {
		const media = lanesFor(mapYaml, 'media');
		for (const file of [
			'crates/abb-engine/src/session/state.rs',
			'crates/abb-metadata-core/src/lib.rs',
		]) {
			expect(media(file), file).toBe(true);
		}
	});

	it('runs the tooling lane, which holds the file-size ratchet, for any authored source', () => {
		const tooling = lanesFor(mapYaml, 'tooling');
		for (const file of [
			'crates/abb-engine/src/lib.rs',
			'src/ui/App.tsx',
			'src-tauri/src/main.rs',
			'scripts/ci-timing.ts',
		]) {
			expect(tooling(file), file).toBe(true);
		}
	});

	const lanes = ['frontend', 'core', 'engine', 'media', 'apple', 'host', 'tooling', 'supply-chain'];
	const selected = (file: string) =>
		requiredLanes(laneMap(mapYaml, lanes), lanes, [file], () => true);

	it('runs only tooling for guidance inside a crate, since no Rust build reads it', () => {
		for (const file of [
			'crates/abb-engine/AGENTS.md',
			'crates/abb-engine/src/session/AGENTS.md',
			'crates/abb-metadata-core/AGENTS.md',
		]) {
			expect(selected(file), file).toEqual(['tooling']);
		}
	});

	it('keeps package.json in host while check-generated-bindings.sh runs a package.json script', () => {
		const bindingsCheck = readFileSync(
			path.join(repoRoot, 'scripts/check-generated-bindings.sh'),
			'utf8',
		);
		expect(bindingsCheck).toContain('bun run bindings:generate');
		expect(selected('package.json')).toContain('host');
		expect(selected('package.json')).not.toContain('engine');
	});

	it('gives every no-lane entry a reason', () => {
		expect(noLaneEntriesWithoutReason(mapYaml)).toEqual([]);
	});

	it('reports a path that no filter names', () => {
		const oldTooling = 'tooling:\n  - .github/workflows/**\n  - scripts/*.sh\n';
		expect(unmappedPaths(oldTooling, ['scripts/verify.sh', '.cursor/environment.json'])).toEqual([
			'.cursor/environment.json',
		]);
	});

	it('reports a no-lane entry without a reason', () => {
		expect(noLaneEntriesWithoutReason('no-lane:\n  LICENSE: legal text\n  deny.toml:\n')).toEqual([
			'deny.toml',
		]);
	});
});
