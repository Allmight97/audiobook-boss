import { createEffect, createSignal, onCleanup, untrack, type Accessor } from 'solid-js';
import type { SettingsOwner } from '../appSettings';
import type { EngineLink } from '../engineLink';
import { toUserMessage } from '../../lib/tauri/appError';
import type { RestartOffer, SessionIntent } from '../../types/session';
import { renderSubmission } from './submit';
import { renderConcurrencyStatus, renderPreview, renderStatus } from './render';
import { coverSrc } from '../../lib/tauri/coverSrc';
import { createStatusViewStore, DEFAULT_STATUS_VIEW, type StatusView } from './view';

export type ProcessingOwner = {
	readonly status: Accessor<StatusView>;
	readonly restartOffers: Accessor<readonly RestartOffer[]>;
	readonly restartPrompt: Accessor<RestartOffer | null>;
	restart: (offer: RestartOffer) => Promise<void>;
	keepLocation: (offer: RestartOffer) => Promise<void>;
	start: () => Promise<void>;
	preview: () => Promise<void>;
	/** Says what Preview Audio does, with the length the engine renders. */
	previewTooltip: () => string;
	cancelAll: () => void;
	cancelJob: (childJobId: string) => void;
	/** The engine still accepts a cancel of the running preview. */
	previewCancellable: () => boolean;
	isProcessing: () => boolean;
	reset: () => void;
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
		showStep: (message: string) => {
			statusView.setStepText(`Current Step: ${message}`);
			statusView.setStepColor('var(--text-primary)');
		},
		showCompleted: (message: string) =>
			renderStatus(statusView, { stage: 'complete', percentage: 100, message }),
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
			renderPreview(statusView, preview);
			if (!preview) return;
			const id = preview.operation.operationId;
			if (artworkKey !== preview.cover) {
				artworkKey = preview.cover;
				statusView.setCoverArtSrc(preview.cover ? coverSrc(preview.cover) : null);
			}
			if (preview.openReady && claiming !== id) {
				claiming = id;
				deps.link.openPreview(id).catch((error: unknown) => {
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
		start() {
			return send({ kind: 'submit' });
		},
		preview() {
			return send({ kind: 'preview' });
		},
		previewTooltip() {
			const { shortestSeconds, longestSeconds } = deps.link.titles().previewLength;
			const length =
				shortestSeconds === longestSeconds
					? `${longestSeconds}-second`
					: `${shortestSeconds}- to ${longestSeconds}-second`;
			return `Encode a ${length} sample of each title, split across its files, with its current audio settings and metadata, so you can listen before exporting.`;
		},
		cancelAll() {
			cancelPreview();
		},
		cancelJob: cancelPreview,
		previewCancellable: () => deps.link.output().previewRun?.operation.cancellable ?? false,
		isProcessing() {
			return deps.link.output().submissionInProgress;
		},
		reset() {
			disposed = true;
			statusView.reset();
		},
	};
}
