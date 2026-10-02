import { createEffect, createSignal, untrack, type Accessor } from 'solid-js';
import type { SettingsOwner } from '../appSettings';
import type { EngineLink } from '../engineLink';
import type { InputOwner } from '../inputSession';
import type { OutputPlanOwner } from '../outputPlan';
import { tauriClient } from '../../lib/tauri/client';
import { toUserMessage } from '../../lib/tauri/appError';
import type { RestartOffer } from '../../types/session';
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
	let submitting = false;
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
	async function start(options?: {
		previewSeconds?: number;
		restart?: RestartOffer;
		resumeReview?: boolean;
	}): Promise<void> {
		if (submitting) return;
		submitting = true;
		try {
			await statusRuntime.startProcessing(options);
		} finally {
			submitting = false;
		}
	}
	// A replacement frontend continues the engine's held review. The same
	// owner already handling a submission does not open a second dialog.
	createEffect(
		() => deps.link.output().submission,
		(submission) => {
			if (submission?.kind === 'reviewRequired' && !submitting) {
				void start({ resumeReview: true });
			}
		},
	);
	// A Save that would move an unfinished export asks once per offer.
	const asked = new Set<string>();
	async function answer(offer: RestartOffer): Promise<void> {
		const restart = await tauriClient.ask(
			`This Save changes where the audiobook goes.\n\nFrom: ${offer.from}\nTo: ${offer.to}\n\n` +
				'Restart it at the new location? Its unfinished output and any empty folders made for it are removed. ' +
				'Keep Location lets the export finish where it is.',
			{ title: 'Restart this export?', okLabel: 'Restart', cancelLabel: 'Keep Location' },
		);
		if (restart) {
			await start({ restart: offer });
		} else {
			await deps.link.send({
				kind: 'keepTitleLocation',
				titleId: offer.titleId,
				revision: offer.revision,
			});
		}
	}
	// One restart runs through review at a time; the next offer is asked
	// once it settles, so two confirmations never compete for submission.
	let answering: Promise<void> = Promise.resolve();
	createEffect(
		() => deps.link.output().restartOffers,
		(offers) => {
			for (const offer of offers) {
				const key = `${offer.titleId}:${offer.revision}`;
				if (asked.has(key)) continue;
				asked.add(key);
				answering = answering
					.then(() => answer(offer))
					.catch((error: unknown) => statusView.showError(toUserMessage(error)));
			}
		},
	);
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
			return start(options);
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
