import { createEffect, createSignal, onCleanup, untrack, type Accessor } from 'solid-js';
import type { SettingsOwner } from '../appSettings';
import type { EngineLink } from '../engineLink';
import { tauriClient } from '../../lib/tauri/client';
import { toUserMessage } from '../../lib/tauri/appError';
import type { RestartOffer, SessionIntent } from '../../types/session';
import { renderSubmission } from './submit';
import { renderConcurrencyStatus, renderPreview, renderStatus } from './render';
import { coverArtBytesToDataUrl } from '../../lib/media/coverArtDataUrl';
import { createStatusViewStore, DEFAULT_STATUS_VIEW, type StatusView } from './view';

/**
 * The engine builds, reviews, and runs exports and previews. This owner
 * starts them, routes collision review through the Output dialog, and shows
 * preview progress and outcomes in the status panel.
 */
export type ProcessingOwner = {
	readonly status: Accessor<StatusView>;
	readonly restartOffers: Accessor<readonly RestartOffer[]>;
	readonly restartPrompt: Accessor<RestartOffer | null>;
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
	let disposed = false;
	onCleanup(() => {
		disposed = true;
		statusView.bindPublisher(null);
	});
	const context = {
		showError: (message: string) => statusView.showError(message),
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
	async function send(intent: SessionIntent): Promise<void> {
		if (disposed) return;
		try {
			await deps.link.send(intent);
		} catch (error) {
			if (!disposed) statusView.showError(`Processing failed: ${toUserMessage(error)}`);
		}
	}
	createEffect(
		() => deps.link.output().submission,
		(submission) => renderSubmission(context, submission),
	);
	createEffect(
		() => deps.link.output().submissionInProgress,
		(active) => deps.settings.setControlsEnabled(!active),
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
		restartPrompt: () => {
			rev();
			return disposed ? null : deps.link.output().restartPrompt;
		},
		restart: (offer) =>
			send({ kind: 'restartTitle', titleId: offer.titleId, revision: offer.revision }),
		keepLocation: (offer) =>
			send({ kind: 'keepTitleLocation', titleId: offer.titleId, revision: offer.revision }),
		status: () => {
			rev();
			return status;
		},
		start(options) {
			return send(
				options?.previewSeconds != null
					? { kind: 'preview', seconds: options.previewSeconds }
					: { kind: 'submit' },
			);
		},
		cancelAll() {
			cancelPreview();
		},
		isProcessing() {
			return deps.link.output().submissionInProgress;
		},
		pushTransientStatus(message, options) {
			statusView.pushTransient(message, options?.ttlMs);
		},
		reset() {
			disposed = true;
			statusView.reset();
		},
	};
}
