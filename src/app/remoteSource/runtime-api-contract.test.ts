import { describe, expect, it } from 'vitest';

import * as remoteSource from '.';

const EXPECTED_APP_REMOTE_SOURCE_EXPORTS = [
	'ORDER_LOCKED_IMPORT_MESSAGE',
	'bytesLabel',
	'cancelRemoteSourceCoverPreviewSchedule',
	'clearRemoteSourceCoverPreviewCache',
	'companionSummaryForInputIds',
	'createRemoteSourceOwner',
	'getRemoteSourceCoverPreviewState',
	'hasSupplementalAssetsForInputId',
	'isAcquisitionTerminal',
	'isTitleAcquirable',
	'progressPercent',
	'progressTitleLabel',
	'purgeRemoteSourceSessionsForInputIds',
	'registerRemoteSourceSupplementalAssets',
	'releaseRemoteSourceSessionRetainers',
	'remoteSourceProviderId',
	'resetRemoteSource',
	'resetRemoteSourceSessionAssets',
	'retainRemoteSourceSessionsForInputIds',
	'runRemoteSourceWorkflow',
	'scheduleRemoteSourceCoverPreviews',
	'selectedRemoteTitleSummaryText',
	'subscribeRemoteSourceCoverPreviews',
	'subscribeRemoteSourceSupplementalAssets',
	'supplementalAssetsByInputIdForProcessing',
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

	it('does not export a workflow Layer factory or private workflow kit symbols', () => {
		expect(remoteSource).not.toHaveProperty('makeRemoteSourceWorkflowServicesLayer');
		expect(remoteSource).not.toHaveProperty('RemoteSourceWorkflowFailed');
		expect(remoteSource).not.toHaveProperty('RemoteSourceWorkflowServicesTag');
		expect(remoteSource).not.toHaveProperty('remoteSourceWorkflowExecution');
	});
});
