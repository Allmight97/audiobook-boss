import { createEffect, createSignal, untrack, type Accessor } from 'solid-js';
import type { SettingsOwner } from '../appSettings';
import type { EngineLink } from '../engineLink';
import type { OutputPlanOwner } from '../outputPlan';
import { tauriClient } from '../../lib/tauri/client';
import { toUserMessage } from '../../lib/tauri/appError';
import type { RestartOffer } from '../../types/session';
import { renderConcurrencyStatus } from './render';
import { runSubmission } from './submit';
import { renderPreview, renderStatus } from './render';
import { coverArtBytesToDataUrl } from '../../lib/media/coverArtDataUrl';
import { onCleanup } from 'solid-js';
import { createStatusViewStore, DEFAULT_STATUS_VIEW, type StatusView } from './view';

/**
 * The engine builds, reviews, and runs exports and previews. This owner
 * starts them, routes collision review through the Output dialog, and shows
 * preview progress and outcomes in the status panel.
 */
export type ProcessingOwner = {
	readonly status: Accessor<StatusView>;
	readonly restartOffers: Accessor<readonly RestartOffer[]>;
	restart(offer: RestartOffer): Promise<void>;
	keepLocation(offer: RestartOffer): Promise<void>;
	start(options?: { previewSeconds?: number }): Promise<void>;
	cancelAll(): void;
	isProcessing(): boolean;
	pushTransientStatus(message: string, options?: { ttlMs?: number }): void;
	reset(): void;
};

export type ProcessingOwnerDeps = {
	readonly link: EngineLink;
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
	let submitting = false;
	let disposed = false;
	onCleanup(() => {
		disposed = true;
		statusView.bindPublisher(null);
	});
	const submit = {
		link: deps.link,
		reviewCollisions: (outputs: Parameters<OutputPlanOwner['openCollisionReview']>[0]) =>
			deps.output.openCollisionReview(outputs),
		setControlsEnabled: (enabled: boolean) => deps.settings.setControlsEnabled(enabled),
		showError: (message: string) => statusView.showError(message),
	};
	const context = {
		updateStatus: (next: Parameters<typeof renderStatus>[1]) =>
			renderStatus(statusView, next, false),
		setProcessingState: (active: boolean) => statusView.setIsProcessing(active),
		handleCancellation: () => statusView.showInfo('Preview was cancelled.'),
		resetToIdle: () => {
			if (!deps.link.output().previewRun) statusView.reset();
		},
	};
	function cancelPreview(childJobId?: string): void {
		const preview = deps.link.output().previewRun;
		if (preview)
			deps.link.post({
				kind: 'cancelPreview',
				runId: preview.operation.operationId,
				childJobId: childJobId ?? null,
			});
	}
	let artworkKey: string | null = null;
	let claiming: string | null = null;
	createEffect(
		() => deps.link.output().previewRun,
		(preview) => {
			renderPreview(statusView, preview, (child) => cancelPreview(child));
			if (!preview) return;
			const id = preview.operation.operationId;
			const key = `${id}:${preview.artworkReady}`;
			if (artworkKey !== key) {
				artworkKey = key;
				statusView.setCoverArtDataUrl(null);
				if (preview.artworkReady) {
					void deps.link
						.send({ kind: 'readPreviewCover', runId: id })
						.then((reply) => {
							if (
								!disposed &&
								deps.link.output().previewRun?.operation.operationId === id &&
								reply.kind === 'previewCover'
							) {
								statusView.setCoverArtDataUrl(
									reply.bytes ? coverArtBytesToDataUrl(reply.bytes) : null,
								);
							}
						})
						.catch((error: unknown) => {
							if (!disposed) console.warn('Preview artwork could not be read:', error);
						});
				}
			}
			if (preview.openReady && claiming !== id) {
				claiming = id;
				void deps.link
					.send({ kind: 'takePreviewOutput', runId: id })
					.then(async (reply) => {
						if (reply.kind === 'previewOutput' && reply.path)
							await tauriClient.openPath(reply.path);
					})
					.catch((error: unknown) => {
						if (!disposed)
							statusView.showError(`Preview could not be opened: ${toUserMessage(error)}`);
					});
			}
		},
	);
	async function start(options?: {
		previewSeconds?: number;
		restart?: RestartOffer;
		resumeReview?: boolean;
	}): Promise<void> {
		if (submitting) return;
		submitting = true;
		try {
			await runSubmission(context, submit, options);
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
		restartOffers: () => deps.link.output().restartOffers,
		restart: (offer) => start({ restart: offer }),
		async keepLocation(offer) {
			try {
				await deps.link.send({
					kind: 'keepTitleLocation',
					titleId: offer.titleId,
					revision: offer.revision,
				});
			} catch (error) {
				statusView.showError(toUserMessage(error));
			}
		},
		status: () => {
			rev();
			return status;
		},
		start(options) {
			return start(options);
		},
		cancelAll() {
			cancelPreview();
		},
		isProcessing() {
			const status = deps.link.output().previewRun?.operation.status;
			return status === 'accepted' || status === 'running' || status === 'cancelling';
		},
		pushTransientStatus(message, options) {
			statusView.pushTransient(message, options?.ttlMs);
		},
		reset() {
			statusView.reset();
		},
	};
}
