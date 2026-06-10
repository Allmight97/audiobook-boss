import { get } from 'svelte/store';

import { getCurrentFileList } from '../fileList';
import { metadataSaveInProgressStore } from '../metadataSaveState';
import {
	initStatusPanel,
	isStatusPanelProcessing,
	pushStatusPanelTransientStatus,
} from '../statusPanel';
import type { MetadataSaveWorkflowEntryServices } from './metadataSaveWorkflow';

export const liveMetadataSaveWorkflowEntryServices: MetadataSaveWorkflowEntryServices = {
	getCurrentFileList,
	initStatusPanel,
	isStatusPanelProcessing,
	pushStatusPanelTransientStatus,
	isMetadataSaveInProgress: () => get(metadataSaveInProgressStore),
	setMetadataSaveInProgress: (isInProgress) => {
		metadataSaveInProgressStore.set(isInProgress);
	},
	console,
};
