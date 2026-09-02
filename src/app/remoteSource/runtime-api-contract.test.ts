import { describe, expect, it } from 'vitest';

import * as remoteSource from '.';

const EXPECTED_APP_REMOTE_SOURCE_EXPORTS = [
	'bytesLabel',
	'createRemoteSourceOwner',
	'isAcquisitionTerminal',
	'isTitleAcquirable',
	'progressPercent',
	'progressTitleLabel',
	'remoteSourceProviderId',
	'selectedRemoteTitleSummaryText',
	'titleAvailability',
	'toggledRemoteTitleSelection',
	'toggledSupplementalPdfPreference',
	'visibleRemoteTitles',
] as const;

describe('app Remote Source Public API Strip', () => {
	it('pins the app remoteSource public export strip', () => {
		expect(Object.keys(remoteSource).sort()).toEqual(
			[...EXPECTED_APP_REMOTE_SOURCE_EXPORTS].sort(),
		);
	});

	it('does not export mutable implementation or workflow symbols', () => {
		expect(remoteSource).not.toHaveProperty('runRemoteSourceWorkflow');
		expect(remoteSource).not.toHaveProperty('resetRemoteSource');
		expect(remoteSource).not.toHaveProperty('subscribeRemoteSourceCoverPreviews');
		expect(remoteSource).not.toHaveProperty('retainRemoteSourceSessionsForInputIds');
		expect(remoteSource).not.toHaveProperty('makeRemoteSourceWorkflowServicesLayer');
		expect(remoteSource).not.toHaveProperty('RemoteSourceWorkflowFailed');
		expect(remoteSource).not.toHaveProperty('RemoteSourceWorkflowServicesTag');
		expect(remoteSource).not.toHaveProperty('remoteSourceWorkflowExecution');
	});
});
