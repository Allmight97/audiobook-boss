import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import picomatch from 'picomatch';
import { describe, expect, it } from 'vitest';
import { isScalar, isSeq, parseDocument } from 'yaml';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const mapPath = path.join(repoRoot, 'scripts/lane-paths.yml');

// Same matcher options as dorny/paths-filter, so a pass here means CI selects a lane.
function unmappedPaths(mapYaml: string, files: string[]): string[] {
	const filters = parseDocument(mapYaml).toJS() as Record<string, unknown[]>;
	const patterns = Object.values(filters).flat(Number.POSITIVE_INFINITY) as string[];
	const isMatch = picomatch(patterns, { dot: true });
	return files.filter((file) => !isMatch(file));
}

function noLaneEntriesWithoutReason(mapYaml: string): string[] {
	const noLane = parseDocument(mapYaml).get('no-lane');
	if (!isSeq(noLane)) {
		return [];
	}
	return noLane.items
		.filter((item) => isScalar(item) && !item.comment?.trim())
		.map((item) => String(isScalar(item) ? item.value : item));
}

describe('scripts/lane-paths.yml', () => {
	const mapYaml = readFileSync(mapPath, 'utf8');

	it('maps every tracked file to a lane or a no-lane entry', () => {
		const tracked = execFileSync('git', ['ls-files'], { cwd: repoRoot, encoding: 'utf8' })
			.split('\n')
			.filter(Boolean);
		expect(
			unmappedPaths(mapYaml, tracked),
			'Add each path to a lane in scripts/lane-paths.yml, or to no-lane with a # reason',
		).toEqual([]);
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
		expect(
			noLaneEntriesWithoutReason('no-lane:\n  - LICENSE # legal text\n  - deny.toml\n'),
		).toEqual(['deny.toml']);
	});
});
