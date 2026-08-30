import { createEffect, createSignal, type Accessor } from 'solid-js';
import type { EncodingRequestConfig } from '../../types/audio';
import type { SettingsOwner } from '../appSettings/owner';
import type { InputOwner } from '../inputSession/owner';
import type { MetadataOwner } from '../metadataSession/owner';
import {
	bindProcessingEncoding,
	bindProcessingInput,
	bindProcessingMetadata,
	bindProcessingSettings,
} from './bind';
import { renderConcurrencyStatus } from './render';
import {
	initStatusPanel,
	isStatusPanelProcessing,
	pushStatusPanelTransientStatus,
	resetStatusPanelRuntime,
	triggerCancelAllFromStatusPanel,
} from './runtime';
import {
	bindStatusPublisher,
	DEFAULT_STATUS_VIEW,
	getStatusView,
	resetStatusPanelViewState,
	type StatusView,
} from './view';

export type ProcessingOwner = {
	readonly status: Accessor<StatusView>;
	start(options?: { previewSeconds?: number }): Promise<void>;
	cancelAll(): void;
	isProcessing(): boolean;
	pushTransientStatus(message: string, options?: { ttlMs?: number }): void;
	reset(): void;
};

export function createProcessingOwner(deps: {
	readonly input: InputOwner;
	readonly metadata: MetadataOwner;
	readonly settings: SettingsOwner;
	readonly encodingRequest: Accessor<EncodingRequestConfig>;
}): ProcessingOwner {
	const [status, setStatus] = createSignal(DEFAULT_STATUS_VIEW);
	bindStatusPublisher(setStatus);
	bindProcessingInput(deps.input);
	bindProcessingMetadata(deps.metadata);
	bindProcessingSettings(deps.settings);
	bindProcessingEncoding(() => deps.encodingRequest());
	resetStatusPanelViewState();
	initStatusPanel();
	createEffect(() => {
		deps.settings.concurrency();
		renderConcurrencyStatus();
	});

	return {
		status,
		start(options) {
			return initStatusPanel().startProcessing(options);
		},
		cancelAll() {
			triggerCancelAllFromStatusPanel();
		},
		isProcessing() {
			return isStatusPanelProcessing();
		},
		pushTransientStatus(message, options) {
			pushStatusPanelTransientStatus(message, options);
		},
		reset() {
			resetStatusPanelRuntime();
			setStatus(getStatusView());
		},
	};
}
