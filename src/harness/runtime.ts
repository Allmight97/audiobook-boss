import type { AudiobookMetadata } from '../types/metadata';
import type { JobListItem } from '../ui/statusPanel/viewTypes';
import { clearMetadataState } from '../ui/metadataState';
import { populateMetadataFormSingle, resetDirtyState } from '../ui/metadataForm';
import {
	updateAbsIncludeYear,
	updateNamingPreset,
	updateNamingTemplate,
	updateOutputDirectory,
	type OutputNamingPreset,
} from '../ui/outputPanel/state';
import { updateNamingOptionState, updateOutputPath } from '../ui/outputPanel/dom';
import { getStatusPanel } from '../ui/statusPanel';
import {
	resetStatusPanelViewState,
	setStatusPanelCancelAllPending,
	setStatusPanelConcurrencyText,
	setStatusPanelJobItems,
	setStatusPanelProgressPercentage,
	setStatusPanelStatusText,
	setStatusPanelStepColor,
	setStatusPanelStepText,
} from '../ui/statusPanel/viewState.svelte';
import { bootstrapHarnessRuntime } from './bootstrap';
import { installHarnessTauriMock, setHarnessCollisionMode } from './mockTauri';

type HarnessOutputSeed = {
	outputDirectory?: string;
	namingPreset?: OutputNamingPreset;
	namingTemplate?: string;
	absIncludeYear?: boolean;
};

type HarnessStatusSeed = {
	jobItems?: JobListItem[];
	progressPercentage?: number;
	statusText?: string;
	stepText?: string;
	stepColor?: string;
	concurrencyText?: string;
	cancelAllPending?: boolean;
};

type HarnessBrowserApi = {
	reset: () => Promise<void>;
	seedMetadata: (metadata: Partial<AudiobookMetadata>) => Promise<void>;
	seedOutput: (seed: HarnessOutputSeed) => Promise<void>;
	seedStatus: (seed: HarnessStatusSeed) => Promise<void>;
	seedCollisionMode: (enabled: boolean) => Promise<void>;
	triggerPreview: (seconds: number) => void;
};

declare global {
	interface Window {
		__ABB_HARNESS__?: HarnessBrowserApi;
		__ABB_HARNESS_READY__?: boolean;
	}
}

function nextFrame(): Promise<void> {
	return new Promise((resolve) => {
		window.requestAnimationFrame(() => resolve());
	});
}

async function settleUi(): Promise<void> {
	await nextFrame();
	await nextFrame();
}

function applyOutputSeed(seed: HarnessOutputSeed): void {
	if (typeof seed.outputDirectory === 'string') {
		updateOutputDirectory(seed.outputDirectory);
	}
	if (seed.namingPreset) {
		updateNamingPreset(seed.namingPreset);
	}
	if (typeof seed.namingTemplate === 'string') {
		updateNamingTemplate(seed.namingTemplate);
	}
	if (typeof seed.absIncludeYear === 'boolean') {
		updateAbsIncludeYear(seed.absIncludeYear);
	}
	updateNamingOptionState();
	updateOutputPath();
}

function applyStatusSeed(seed: HarnessStatusSeed): void {
	if (seed.jobItems) {
		setStatusPanelJobItems(seed.jobItems);
	}
	if (typeof seed.progressPercentage === 'number') {
		setStatusPanelProgressPercentage(seed.progressPercentage);
	}
	if (typeof seed.statusText === 'string') {
		setStatusPanelStatusText(seed.statusText);
	}
	if (typeof seed.stepText === 'string') {
		setStatusPanelStepText(seed.stepText);
	}
	if (typeof seed.stepColor === 'string') {
		setStatusPanelStepColor(seed.stepColor);
	}
	if (typeof seed.concurrencyText === 'string') {
		setStatusPanelConcurrencyText(seed.concurrencyText);
	}
	if (typeof seed.cancelAllPending === 'boolean') {
		setStatusPanelCancelAllPending(seed.cancelAllPending);
	}
}

export function installHarnessRuntime(): void {
	if (typeof window === 'undefined') return;

	installHarnessTauriMock();
	window.__ABB_HARNESS_READY__ = false;

	const harnessReady = bootstrapHarnessRuntime().then(() => {
		window.__ABB_HARNESS_READY__ = true;
	});

	window.__ABB_HARNESS__ = {
		reset: async () => {
			await harnessReady;
			clearMetadataState();
			populateMetadataFormSingle({});
			resetDirtyState();
			resetStatusPanelViewState();
			applyOutputSeed({
				outputDirectory: '',
				namingPreset: 'absDefault',
				namingTemplate: '',
				absIncludeYear: false,
			});
			setHarnessCollisionMode(false);
			await settleUi();
		},
		seedMetadata: async (metadata) => {
			await harnessReady;
			populateMetadataFormSingle(metadata);
			updateOutputPath();
			await settleUi();
		},
		seedOutput: async (seed) => {
			await harnessReady;
			applyOutputSeed(seed);
			await settleUi();
		},
		seedStatus: async (seed) => {
			await harnessReady;
			applyStatusSeed(seed);
			await settleUi();
		},
		seedCollisionMode: async (enabled) => {
			await harnessReady;
			setHarnessCollisionMode(enabled);
			await settleUi();
		},
		triggerPreview: (seconds) => {
			void harnessReady.then(() => getStatusPanel()?.startProcessing({ previewSeconds: seconds }));
		},
	};
	void harnessReady;
}
