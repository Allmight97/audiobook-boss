import { describe, expect, it } from 'vitest';

import { planGitHubRelease, type ReleaseObservation } from './github-publish';

const head = 'a'.repeat(40);
const other = 'b'.repeat(40);

function observed(overrides: Partial<ReleaseObservation> = {}): ReleaseObservation {
	return {
		tag: 'v1.20.0',
		headSha: head,
		tagSha: null,
		release: null,
		dmgAsset: 'AudioBook.Boss_1.20.0_aarch64.dmg',
		checksumAsset: 'AudioBook.Boss_1.20.0_aarch64.dmg.sha256',
		...overrides,
	};
}

describe('GitHub release plan', () => {
	it('builds and creates the tag when the version has no tag', () => {
		expect(planGitHubRelease(observed())).toMatchObject({ action: 'build', createTag: true });
	});

	it('skips a published release so a rerun does not publish again', () => {
		const plan = planGitHubRelease(
			observed({
				tagSha: head,
				release: {
					draft: false,
					assetNames: [
						'AudioBook.Boss_1.20.0_aarch64.dmg',
						'AudioBook.Boss_1.20.0_aarch64.dmg.sha256',
					],
				},
			}),
		);
		expect(plan.action).toBe('skip');
	});

	it('finishes a tag on this commit without creating the tag again', () => {
		expect(planGitHubRelease(observed({ tagSha: head, release: null }))).toMatchObject({
			action: 'build',
			createTag: false,
		});
	});

	it('does not retag or republish when the tag points at another commit', () => {
		expect(planGitHubRelease(observed({ tagSha: other })).action).toBe('skip');
		expect(
			planGitHubRelease(
				observed({
					tagSha: other,
					release: { draft: true, assetNames: [] },
				}),
			).action,
		).toBe('skip');
	});

	it('refuses a published release that is missing its checksum', () => {
		const plan = planGitHubRelease(
			observed({
				tagSha: head,
				release: { draft: false, assetNames: ['AudioBook.Boss_1.20.0_aarch64.dmg'] },
			}),
		);
		expect(plan).toMatchObject({
			action: 'refuse',
			message: expect.stringMatching(/immutable/),
		});
		expect(plan.message).toContain('Fix:');
		expect(plan.message).toContain('will not publish a second release');
	});
});
