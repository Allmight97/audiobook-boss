import { createEffect, createSignal, untrack, type Accessor } from 'solid-js';
import type { SettingsOwner } from '../appSettings';
import type { EncodingOwner } from '../encoding';
import type { InputOwner } from '../inputSession';
import type { MetadataOwner } from '../metadataSession';
import type { RemoteSourceOwner } from '../remoteSource';
import { bindProcessingInput, bindProcessingMetadata, bindProcessingSettings } from './bind';
import { setProcessingEncodingReader } from './config';
import { renderConcurrencyStatus } from './render';
import { pushStatusPanelTransientStatus, StatusPanelRuntime } from './runtime';
import { makeProcessingWorkflowLive } from './workflow.deps';
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
	readonly encoding: Pick<EncodingOwner, 'request'>;
	readonly remoteSource: Pick<RemoteSourceOwner, 'processingAssets' | 'withSubmissionRetention'>;
}): ProcessingOwner {
	let status = DEFAULT_STATUS_VIEW;
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	function publish(next: StatusView): void {
		status = next;
		bump((n) => n + 1);
	}
	bindStatusPublisher(publish);
	bindProcessingInput(deps.input);
	bindProcessingMetadata(deps.metadata);
	bindProcessingSettings(deps.settings);
	setProcessingEncodingReader(() => deps.encoding.request());
	resetStatusPanelViewState();
	const statusRuntime = new StatusPanelRuntime(makeProcessingWorkflowLive(deps.remoteSource));
	createEffect(
		() => deps.settings.concurrency(),
		() => {
			untrack(() => {
				renderConcurrencyStatus();
			});
		},
	);

	return {
		status: () => {
			rev();
			return status;
		},
		start(options) {
			return statusRuntime.startProcessing(options);
		},
		cancelAll() {
			statusRuntime.requestCancelAll();
		},
		isProcessing() {
			return statusRuntime.isCurrentlyProcessing;
		},
		pushTransientStatus(message, options) {
			pushStatusPanelTransientStatus(message, options);
		},
		reset() {
			statusRuntime.resetToIdle();
			publish(getStatusView());
		},
	};
}
