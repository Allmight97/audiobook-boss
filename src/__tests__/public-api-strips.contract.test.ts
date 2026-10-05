import { describe, expect, it } from 'vitest';

import * as appSettings from '../app/appSettings';
import * as encoding from '../app/encoding';
import * as inputSession from '../app/inputSession';
import * as metadataLookup from '../app/metadataLookup';
import * as outputPlan from '../app/outputPlan';
import * as processing from '../app/processing';
import * as remoteSource from '../app/remoteSource';
import * as workOperations from '../app/workOperations';
import * as appSettingsUi from '../ui/appSettings';
import * as fileImport from '../ui/fileImport';
import * as fileList from '../ui/fileList';
import * as leftColumn from '../ui/leftColumn';
import * as outputPanel from '../ui/outputPanel';
import * as remoteSourceUi from '../ui/remoteSource';
import * as statusPanel from '../ui/statusPanel';
import * as tagPreview from '../ui/tagPreview';
import * as workCenter from '../ui/workCenter';

// Each owner's index.ts is its exact Public API Strip (src/app/AGENTS.md, src/ui/AGENTS.md).
// The expected lists are written here by hand, independent of the modules.
const STRIPS: ReadonlyArray<readonly [string, object, readonly string[]]> = [
	['app/appSettings', appSettings, ['createSettingsOwner']],
	['app/encoding', encoding, ['createEncodingOwner']],
	[
		'app/inputSession',
		inputSession,
		[
			'createInputOwner',
			'displayedArtistForFile',
			'displayedTitleForFile',
			'fileListNavigationCommandFromKey',
			'formatAudioProperties',
			'formatFileDetails',
			'interpretFileListKeyDown',
			'nativeDropTargetAtPoint',
			'resolveFileListNavigationTarget',
			'toInputView',
			'toInspectorViewFromInput',
		],
	],
	['app/metadataLookup', metadataLookup, ['createMetadataLookupOwner']],
	['app/outputPlan', outputPlan, ['CUSTOM_TEMPLATE_PLACEHOLDER', 'createOutputOwner']],
	['app/processing', processing, ['createProcessingOwner']],
	[
		'app/remoteSource',
		remoteSource,
		[
			'bytesLabel',
			'createRemoteSourceOwner',
			'formatReleaseSizeBytes',
			'isTitleAcquirable',
			'progressPercent',
			'progressTitleLabel',
			'releaseKey',
			'releaseProtocolLabel',
			'selectedRemoteTitleSummaryText',
			'visibleRemoteReleases',
			'visibleRemoteTitles',
		],
	],
	['app/workOperations', workOperations, ['createWorkOperationsOwner']],
	['ui/appSettings', appSettingsUi, ['AppSettingsDialogView', 'SettingsPersistenceNotice']],
	['ui/fileImport', fileImport, ['FileImportView']],
	['ui/fileList', fileList, ['FileListView', 'SelectedAudioSettings']],
	['ui/leftColumn', leftColumn, ['FileInspectorView']],
	['ui/outputPanel', outputPanel, ['OutputView']],
	['ui/remoteSource', remoteSourceUi, ['RemoteSourceAcquireView']],
	['ui/statusPanel', statusPanel, ['StatusPanelView']],
	['ui/tagPreview', tagPreview, ['TagPreviewView']],
	['ui/workCenter', workCenter, ['WorkCenterView']],
];

describe('owner Public API Strips', () => {
	it.each(STRIPS)('%s exports exactly its strip', (_owner, module, expected) => {
		expect(Object.keys(module).sort()).toEqual([...expected].sort());
	});
});
