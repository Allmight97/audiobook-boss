import {
	createEffect,
	createMemo,
	createSignal,
	onCleanup,
	untrack,
	type Accessor,
} from 'solid-js';
import type { SettingsOwner } from '../appSettings';
import type { EngineLink } from '../engineLink';
import { toUserMessage } from '../../lib/tauri/appError';
import type { OutputSnapshot, RestartOffer, SessionIntent } from '../../lib/generated/tauri';
import { processingFailure } from './submit';
import { statusOf } from './status';
import type { StatusView } from './view';

type LocalError = {
	readonly message: string;
	readonly previewRun: OutputSnapshot['previewRun'];
	readonly submission: OutputSnapshot['submission'];
};

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
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	const [localError, setLocalError] = createSignal<LocalError | null>(null, { ownedWrite: true });
	let disposed = false;
	onCleanup(() => {
		disposed = true;
	});
	function showError(message: string): void {
		if (disposed) return;
		const { previewRun, submission } = untrack(() => deps.link.output());
		setLocalError({ message, previewRun, submission });
	}
	const status = createMemo(() => {
		const { previewRun, submission } = deps.link.output();
		const error = localError();
		return statusOf({
			previewRun,
			submission,
			localError:
				error?.previewRun === previewRun && error.submission === submission ? error.message : null,
			concurrency: deps.settings.concurrency(),
		});
	});
	function cancelPreview(childJobId?: string): void {
		const preview = deps.link.output().previewRun;
		if (preview)
			deps.link.post({
				kind: 'cancelPreview',
				runId: preview.operation.operationId,
				childJobId: childJobId ?? null,
			});
	}
	let claiming: string | null = null;
	createEffect(
		() => deps.link.output().previewRun,
		(preview) => {
			if (!preview?.openReady) return;
			const id = preview.operation.operationId;
			if (claiming === id) return;
			claiming = id;
			deps.link.openPreview(id).catch((error: unknown) => {
				showError(`Preview could not be opened: ${toUserMessage(error)}`);
			});
		},
	);
	async function send(intent: SessionIntent): Promise<void> {
		if (disposed) return;
		try {
			await deps.link.send(intent);
		} catch (error) {
			showError(processingFailure(error));
		}
	}
	createEffect(
		() => deps.link.output().submissionInProgress,
		(active) => deps.settings.setControlsEnabled(!active),
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
		status,
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
			bump((n) => n + 1);
		},
	};
}
