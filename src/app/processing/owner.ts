import { createEffect, createSignal, untrack, type Accessor } from 'solid-js';
import type { SettingsOwner } from '../appSettings';
import type { EngineLink } from '../engineLink';
import type { InputOwner } from '../inputSession';
import type { OutputPlanOwner } from '../outputPlan';
import { renderConcurrencyStatus } from './render';
import { StatusPanelRuntime } from './runtime';
import { createStatusViewStore, DEFAULT_STATUS_VIEW, type StatusView } from './view';

/**
 * The engine builds, reviews, and runs exports and previews. This owner
 * starts them, routes collision review through the Output dialog, and shows
 * preview progress and outcomes in the status panel.
 */
export type ProcessingOwner = {
	readonly status: Accessor<StatusView>;
	start(options?: { previewSeconds?: number }): Promise<void>;
	cancelAll(): void;
	isProcessing(): boolean;
	pushTransientStatus(message: string, options?: { ttlMs?: number }): void;
	reset(): void;
};

export type ProcessingOwnerDeps = {
	readonly link: EngineLink;
	readonly input: InputOwner;
	readonly settings: SettingsOwner;
	readonly output: Pick<OutputPlanOwner, 'openCollisionReview'>;
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
	const validTitles = () => deps.input.view().files.filter((file) => file.isValid);
	const statusRuntime = new StatusPanelRuntime({
		view: statusView,
		validTitles,
		unlockWorkbench: () => deps.settings.setControlsEnabled(true),
		concurrency: () => untrack(deps.settings.concurrency),
		submit: {
			link: deps.link,
			reviewCollisions: (outputs) => deps.output.openCollisionReview(outputs),
			titlePaths: () => validTitles().map((file) => file.path),
			setControlsEnabled: (enabled) => deps.settings.setControlsEnabled(enabled),
			showError: (message) => statusView.showError(message),
		},
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
