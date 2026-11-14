import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

import {
	bundleResourceProblems,
	bundleResources,
	changelogSection,
	type CiRun,
	ciFailure,
	ciRunFor,
	LANES,
	parseArgs,
	stepsFor,
	unexpectedChanges,
} from './release';

describe('arguments', () => {
	it('takes a version and a lane', () => {
		expect(parseArgs(['1.20.0', 'github+local'])).toEqual({
			version: '1.20.0',
			lane: 'github+local',
		});
	});

	it('refuses a missing lane, an unknown lane, a v-prefixed version, and extra words', () => {
		expect(() => parseArgs(['1.20.0'])).toThrow('Usage');
		expect(() => parseArgs(['1.20.0', 'public'])).toThrow("Unknown lane 'public'");
		expect(() => parseArgs(['v1.20.0', 'dmg'])).toThrow("must be x.y.z, got 'v1.20.0'");
		expect(() => parseArgs(['1.20.0', 'dmg', 'now'])).toThrow('Usage');
	});
});

describe('lane steps', () => {
	it('tags only after CI passed on the pushed release commit', () => {
		for (const lane of LANES) {
			const steps = stepsFor(lane);
			if (!steps.includes('tag')) continue;
			expect(steps.indexOf('push main')).toBeLessThan(steps.indexOf('wait for ci'));
			expect(steps.indexOf('wait for ci')).toBeLessThan(steps.indexOf('tag'));
		}
	});

	it('pushes nothing from the local and dmg lanes', () => {
		for (const lane of ['local', 'dmg'] as const) {
			expect(stepsFor(lane)).not.toContain('push main');
			expect(stepsFor(lane)).not.toContain('tag');
		}
	});

	it('installs in github+local only after the DMG passed its checks, before anything is pushed', () => {
		const steps = stepsFor('github+local');
		expect(steps.indexOf('check bundled licenses')).toBeLessThan(steps.indexOf('install app'));
		expect(steps.indexOf('install app')).toBeLessThan(steps.indexOf('commit'));
	});
});

describe('changelog section', () => {
	const changelog = [
		'# Changelog',
		'',
		'## [Unreleased]',
		'',
		'## [1.20.0] - 2026-11-14',
		'',
		'### Fixed',
		'',
		'- A fix.',
		'',
		'## [1.19.1] - 2026-11-13',
		'',
		'### Fixed',
		'',
		'- Older.',
	].join('\n');

	it("returns the section's notes without its heading or the next section", () => {
		expect(changelogSection(changelog, '1.20.0', '2026-11-14')).toBe('### Fixed\n\n- A fix.\n');
	});

	it('refuses a missing section, a section dated another day, and a section with no entries', () => {
		expect(() => changelogSection(changelog, '1.20.1', '2026-11-14')).toThrow(
			"no '## [1.20.1] - 2026-11-14' section",
		);
		expect(() => changelogSection(changelog, '1.20.0', '2026-11-15')).toThrow(
			"must read '## [1.20.0] - 2026-11-15'",
		);
		const empty = changelog.replace('- A fix.', '');
		expect(() => changelogSection(empty, '1.20.0', '2026-11-14')).toThrow('has no entries');
	});
});

describe('tree check', () => {
	it('allows only the changelog and the version files', () => {
		const porcelain = [
			' M CHANGELOG.md',
			' M package.json',
			' M src-tauri/tauri.conf.json',
			' M src-tauri/Cargo.toml',
			' M Cargo.lock',
			' M scripts/release.ts',
			'?? notes.txt',
		].join('\n');
		expect(unexpectedChanges(porcelain)).toEqual(['scripts/release.ts', 'notes.txt']);
	});
});

describe('CI run', () => {
	const sha = 'a'.repeat(40);
	const ciRun = (overrides: Partial<CiRun>): CiRun => ({
		databaseId: 1,
		headSha: sha,
		event: 'push',
		status: 'completed',
		conclusion: 'success',
		url: 'https://example.invalid/run',
		...overrides,
	});

	it('refuses a short hash, which gh run list --commit silently matches to nothing', () => {
		expect(() => ciRunFor([ciRun({})], 'aaaaaaa')).toThrow('full 40-character commit SHA');
	});

	it("picks the newest push run of the release commit, not a scheduled run or another commit's", () => {
		const runs = [
			ciRun({ databaseId: 5, event: 'schedule' }),
			ciRun({ databaseId: 4, headSha: 'b'.repeat(40) }),
			ciRun({ databaseId: 2 }),
			ciRun({ databaseId: 3 }),
		];
		expect(ciRunFor(runs, sha)?.databaseId).toBe(3);
		expect(ciRunFor([], sha)).toBeUndefined();
	});

	it('names a later push as the reason a cancelled run blocks the tag', () => {
		const later = 'c'.repeat(40);
		expect(ciFailure(ciRun({ conclusion: 'cancelled' }), later)).toContain(
			`cancelled because a later push replaced it: origin/main is now ${later}`,
		);
		expect(ciFailure(ciRun({ conclusion: 'cancelled' }), sha)).toContain('gh run rerun 1');
		expect(ciFailure(ciRun({ conclusion: 'failure' }), sha)).toContain("ended 'failure'");
		expect(ciFailure(ciRun({}), sha)).toBeUndefined();
	});
});

describe('bundled licenses', () => {
	const scratch: string[] = [];
	afterEach(() => {
		for (const dir of scratch.splice(0)) rmSync(dir, { recursive: true, force: true });
	});

	function layout() {
		const root = mkdtempSync(path.join(os.tmpdir(), 'abb-release-licenses-'));
		scratch.push(root);
		mkdirSync(path.join(root, 'src-tauri'));
		mkdirSync(path.join(root, 'vendor/lib'), { recursive: true });
		writeFileSync(path.join(root, 'vendor/lib/COPYING'), 'license text');
		writeFileSync(path.join(root, 'vendor/lib/NOTICE.md'), 'notice text');
		const resourcesDir = path.join(root, 'App.app/Contents/Resources');
		mkdirSync(path.join(resourcesDir, 'licenses'), { recursive: true });
		const resources = bundleResources({
			bundle: {
				resources: {
					'../vendor/lib/COPYING': 'licenses/Lib.txt',
					'../vendor/lib/NOTICE.md': 'licenses/Lib-source.md',
				},
			},
		});
		const check = () =>
			bundleResourceProblems(resources, path.join(root, 'src-tauri'), resourcesDir);
		return { resourcesDir, check };
	}

	it('passes when every listed resource ships with its source bytes', () => {
		const { resourcesDir, check } = layout();
		writeFileSync(path.join(resourcesDir, 'licenses/Lib.txt'), 'license text');
		writeFileSync(path.join(resourcesDir, 'licenses/Lib-source.md'), 'notice text');
		expect(check()).toEqual([]);
	});

	it('names a missing resource and one that differs from its source', () => {
		const { resourcesDir, check } = layout();
		writeFileSync(path.join(resourcesDir, 'licenses/Lib-source.md'), 'stale notice');
		expect(check()).toEqual([
			'licenses/Lib.txt is missing (from ../vendor/lib/COPYING)',
			'licenses/Lib-source.md differs from ../vendor/lib/NOTICE.md',
		]);
	});

	it('refuses a resources list without targets', () => {
		expect(() => bundleResources({ bundle: { resources: ['../COPYING'] } })).toThrow(
			'must map each source to its target',
		);
	});
});
