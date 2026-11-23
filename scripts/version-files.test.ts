import { describe, expect, it } from 'vitest';

import { VERSION_SURFACES } from './bump-version';
import { latestStableTag, versionReleaseFindings, type VersionSurface } from './version-files';

function versions(version: string): VersionSurface[] {
	return VERSION_SURFACES.map((file) => ({ file, version }));
}

const older = [
	'# Changelog',
	'',
	'## [Unreleased]',
	'',
	'## [1.19.2] - 2026-11-20',
	'',
	'- Older.',
].join('\n');

function withSection(heading: string): string {
	return [
		'# Changelog',
		'',
		'## [Unreleased]',
		'',
		heading,
		'',
		'### Fixed',
		'',
		'- A fix.',
		'',
		'## [1.19.2] - 2026-11-20',
		'',
		'- Older.',
	].join('\n');
}

describe('version release check', () => {
	it('a version bump without a changelog section fails', () => {
		const missing = versionReleaseFindings({
			versions: versions('1.20.0'),
			changelog: older,
			tags: ['v1.19.2'],
		});
		expect(missing).toHaveLength(1);
		expect(missing[0]?.message).toContain("no non-empty '## [1.20.0]'");
		expect(missing[0]?.message).toContain('Fix:');

		const empty = versionReleaseFindings({
			versions: versions('1.20.0'),
			changelog: ['# Changelog', '', '## [Unreleased]', '', '## [1.20.0] - 2020-01-02', ''].join(
				'\n',
			),
			tags: ['v1.19.2'],
		});
		expect(empty[0]?.message).toContain("section '## [1.20.0]' is empty");
		expect(empty[0]?.message).toContain('Fix:');
	});

	it('a changelog section for the bumped version passes', () => {
		for (const heading of ['## [1.20.0] - 2020-01-02', '## [1.20.0]']) {
			expect(
				versionReleaseFindings({
					versions: versions('1.20.0'),
					changelog: withSection(heading),
					tags: ['v1.19.1', 'v1.19.2'],
				}),
			).toEqual([]);
		}
	});

	it('mismatched version files fail', () => {
		const surfaces = versions('1.20.0');
		const cargo = surfaces.find((surface) => surface.file === 'src-tauri/Cargo.toml');
		expect(cargo).toBeDefined();
		if (cargo) cargo.version = '1.19.2';
		const findings = versionReleaseFindings({
			versions: surfaces,
			changelog: withSection('## [1.20.0] - 2020-01-02'),
			tags: ['v1.19.2'],
		});
		expect(findings).toHaveLength(1);
		expect(findings[0]?.message).toContain('Version files disagree');
		expect(findings[0]?.message).toContain('src-tauri/Cargo.toml is 1.19.2');
		expect(findings[0]?.message).toContain('bun scripts/bump-version.ts');
		expect(findings[0]?.message).toContain('Fix:');
	});

	it('a checkout with no vX.Y.Z tags fails', () => {
		const findings = versionReleaseFindings({
			versions: versions('1.19.2'),
			changelog: older,
			tags: [],
		});
		expect(findings).toHaveLength(1);
		expect(findings[0]?.message).toContain('Fix: fetch the tags');
		expect(findings[0]?.message).toContain('depth-1');
	});

	it('an unchanged version needs no changelog section', () => {
		expect(
			versionReleaseFindings({
				versions: versions('1.10.0'),
				changelog: '# Changelog\n\n## [Unreleased]\n',
				tags: ['v1.9.0', 'v1.10.0'],
			}),
		).toEqual([]);
		expect(latestStableTag(['v1.9.0', 'v1.10.0', 'v1.10.0-beta.1'])).toBe('v1.10.0');
	});
});
