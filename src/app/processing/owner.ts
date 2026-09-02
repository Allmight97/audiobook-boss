import { createEffect, createSignal, untrack, type Accessor } from 'solid-js';
import type { SettingsOwner } from '../appSettings';
import type { EncodingOwner } from '../encoding';
import type { InputOwner } from '../inputSession';
import type { MetadataOwner } from '../metadataSession';
import type { OutputPlanOwner } from '../outputPlan';
import type { RemoteSourceOwner } from '../remoteSource';
import { fileListFromInput } from './input';
import { renderConcurrencyStatus } from './render';
import { StatusPanelRuntime } from './runtime';
import { makeProcessingWorkflowLive } from './workflow.deps';
import { createStatusViewStore, DEFAULT_STATUS_VIEW, type StatusView } from './view';

export type ProcessingOwner = {
	readonly status: Accessor<StatusView>;
	start(options?: { previewSeconds?: number }): Promise<void>;
	cancelAll(): void;
	isProcessing(): boolean;
	pushTransientStatus(message: string, options?: { ttlMs?: number }): void;
	reset(): void;
};

export type ProcessingOwnerDeps = {
	readonly input: InputOwner;
	readonly metadata: MetadataOwner;
	readonly settings: SettingsOwner;
	readonly encoding: Pick<EncodingOwner, 'request'>;
	readonly output: Pick<OutputPlanOwner, 'readRequestConfig'>;
	readonly remoteSource: Pick<RemoteSourceOwner, 'processingAssets' | 'withSubmissionRetention'>;
};

export function createProcessingOwner(deps: ProcessingOwnerDeps): ProcessingOwner {
	let status = DEFAULT_STATUS_VIEW;
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	function publish(next: StatusView): void {
		status = next;
		bump((n) => n + 1);
	}
	const statusView = createStatusViewStore();
	statusView.bindPublisher(publish);
	const statusRuntime = new StatusPanelRuntime({
		view: statusView,
		getCurrentFileList: () => fileListFromInput(deps.input.view()),
		unlockWorkbench: () => {
			deps.settings.setControlsEnabled(true);
			deps.input.setOrderLocked(false);
		},
		concurrency: () => deps.settings.concurrency(),
		workflowLayer: makeProcessingWorkflowLive({
			input: deps.input,
			metadata: deps.metadata,
			settings: deps.settings,
			encoding: deps.encoding,
			output: deps.output,
			remoteSource: deps.remoteSource,
			showError: (message) => statusView.showError(message),
		}),
	});
	createEffect(
		() => deps.settings.concurrency(),
		(concurrency) => {
			untrack(() => {
				renderConcurrencyStatus(statusView, concurrency);
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
			statusView.pushTransient(message, options?.ttlMs);
		},
		reset() {
			statusRuntime.resetToIdle();
			statusView.reset();
		},
	};
}
