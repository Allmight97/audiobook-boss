import { describe, expect, it } from 'vitest';

import * as remoteSource from '.';

const EXPECTED_APP_REMOTE_SOURCE_EXPORTS = [
	'bytesLabel',
	'createRemoteSourceOwner',
	'formatReleaseSizeBytes',
	'isAcquisitionTerminal',
	'isTitleAcquirable',
	'progressPercent',
	'progressTitleLabel',
	'providerIdFromLane',
	'releaseProtocolLabel',
	'selectedRemoteTitleSummaryText',
	'titleAvailability',
	'toggledRemoteTitleSelection',
	'toggledSupplementalPdfPreference',
	'visibleRemoteReleases',
	'visibleRemoteTitles',
] as const;

describe('app Remote Source Public API Strip', () => {
	it('pins the app remoteSource public export strip', () => {
		expect(Object.keys(remoteSource).sort()).toEqual(
			[...EXPECTED_APP_REMOTE_SOURCE_EXPORTS].sort(),
		);
	});
});
