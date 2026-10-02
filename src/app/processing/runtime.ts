import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../types/events';
import type { AudioFile, ProcessCommandResult } from '../../types/audio';
import { buildQueueLabels, extractFilenameFromProgress } from './formatting';
import type { RestartOffer } from '../../types/session';
import { runSubmission, type SubmitDeps } from './submit';
import {
	renderConcurrencyStatus,
	renderJobList,
	renderStatus,
	type ConcurrencyRead,
} from './render';
import type { AggregateProgress, ProcessingStatus } from './state';
import { calculateAggregateProgressAndStage } from './domain/aggregate';
import { buildJobKey as buildJobKeyDomain } from './domain/jobKeys';
import { createCoverArtTracker } from './services/coverArtTracker';
import {
	findFilePathByIndex as findFilePathByIndexService,
	findFilePathByCurrentFile as findFilePathByCurrentFileService,
} from './services/fileLookup';
import { createProgressSubscription } from './services/progressSubscription';
import {
	applyCancellation,
	applyProgress,
	applyQueueSnapshot,
	completeBatchCompletionHold,
	completeSingleCompletionHold,
	createStatusPanelModel,
	isTerminalProgressStage,
	reconcileProcessResult,
	resetStatusPanelModel,
	withBatchCompletionMessage,
	type StatusPanelCompletionFeedback,
	type StatusPanelIntent,
	type StatusPanelModel,
} from './domain/stateMachine';
import type { StatusViewStore } from './view';

export type StatusPanelRuntimeDeps = {
	readonly view: StatusViewStore;
	readonly validTitles?: () => ReadonlyArray<AudioFile>;
	readonly unlockWorkbench?: () => void;
	readonly concurrency?: () => ConcurrencyRead | undefined;
	readonly submit?: SubmitDeps;
};

export class StatusPanelRuntime {
	private readonly view: StatusViewStore;
	private readonly readValidTitles: () => ReadonlyArray<AudioFile>;
	private readonly unlockWorkbench: () => void;
	private readonly readConcurrency: () => ConcurrencyRead | undefined;
	private readonly submit?: SubmitDeps;
	private readonly progressSubscription = createProgressSubscription({
		onProgress: (event) => this.updateProgress(event),
		onQueue: (event) => this.handleQueueSnapshot(event),
	});
	private readonly coverArt;
	private model: StatusPanelModel;
	private batchCompletionTimeout?: number;
	private singleCompletionTimeout?: number;
	private pendingRender = false;

	constructor(deps: StatusPanelRuntimeDeps) {
		this.view = deps.view;
		this.readValidTitles = deps.validTitles ?? (() => []);
		this.unlockWorkbench = deps.unlockWorkbench ?? (() => undefined);
		this.readConcurrency = deps.concurrency ?? (() => undefined);
		this.submit = deps.submit;
		this.coverArt = createCoverArtTracker({
			validTitles: () => this.readValidTitles(),
			displayCoverArt: (dataUrl) => this.view.setCoverArtDataUrl(dataUrl),
			resetArtThumbnail: () => this.view.setCoverArtDataUrl(null),
		});
		this.model = createStatusPanelModel();
		this.renderModel();
		this.updateConcurrencyIndicator();
		this.coverArt.reset();
	}

	public async startProcessing(options?: {
		previewSeconds?: number;
		restart?: RestartOffer;
		resumeReview?: boolean;
	}): Promise<void> {
		this.clearSingleCompletionTimeout();
		this.clearBatchCompletionTimeout();

		if (!this.submit) {
			return Promise.reject(new Error('Processing requires its engine link.'));
		}
		return runSubmission(
			{
				updateStatus: (status) => this.updateStatus(status),
				setProcessingState: (isProcessing) => {
					this.model = {
						...this.model,
						isProcessing,
						...(isProcessing ? { cancellationLatched: false } : {}),
					};
				},
				updateArtThumbnail: () => this.coverArt.syncForCurrentList(),
				startProgressListener: () => this.progressSubscription.start(),
				setBatchCompletionMessage: (message) => this.setBatchCompletionMessage(message),
				reconcileProcessResult: (result) => this.reconcileProcessResult(result),
				handleCancellation: () => this.handleProcessingCancellation(),
				resetToIdle: () => this.resetToIdle(),
			},
			this.submit,
			options,
		);
	}

	public setBatchCompletionMessage(message: string | null): void {
		this.model = withBatchCompletionMessage(this.model, message);
	}

	public reconcileProcessResult(result: ProcessCommandResult): void {
		const transition = reconcileProcessResult(this.model, result, Date.now());
		this.model = transition.model;
		this.renderModel();
		this.handleIntents(transition.intents);
	}

	public applyQueueSnapshot(event: ProcessingQueueEvent): void {
		const transition = applyQueueSnapshot(this.model, event, Date.now());
		if (transition.model === this.model && transition.intents.length === 0) {
			return;
		}
		this.clearBatchCompletionTimeout();
		this.clearSingleCompletionTimeout();

		this.model = transition.model;
		this.renderModel();
	}

	public applyProgress(event: ProcessingProgressEvent): void {
		const jobKey = this.buildJobKey(event.input_index, event.job_id ?? undefined);
		const existing = this.model.jobProgress.get(jobKey);
		const label = existing?.label ?? this.buildInferredProgressLabel(event);
		const transition = applyProgress(this.model, event, Date.now(), { label });

		if (transition.model === this.model && transition.intents.length === 0) {
			return;
		}
		this.model = transition.model;
		this.handleIntents(transition.intents);
		this.scheduleRender(isTerminalProgressStage(event.stage));
	}

	public requestCancelAll(): void {
		// Foreground/direct cancellation is operation-scoped only at the backend
		// (Work Center → cancel_work_operation). The retained foreground lane is
		// preview rendering, which has no backend cancel command. The cancel-all
		// button stays in the UI; with an in-flight foreground job it settles the
		// local render. The backend preview, if any, completes and auto-opens
		// normally. Preview is an ephemeral render lane, never a Work Center row.
		if (this.cancellableForegroundJobIds().length === 0) {
			return;
		}
		this.handleProcessingCancellation();
	}

	public handleProcessingCancellation(): void {
		if (this.batchCompletionTimeout || this.singleCompletionTimeout) {
			return;
		}

		if (this.model.jobProgress.size === 0) {
			this.view.showInfo('Processing was cancelled.');
			this.resetToIdle();
			return;
		}

		const transition = applyCancellation(this.model, Date.now());
		this.model = transition.model;
		this.handleIntents(transition.intents);
		this.scheduleRender(true);
	}

	public resetToIdle(): void {
		this.model = resetStatusPanelModel();

		this.progressSubscription.stop();
		this.clearBatchCompletionTimeout();
		this.clearSingleCompletionTimeout();

		this.pendingRender = false;
		renderJobList(this.view, this.model.jobProgress, this.model.queueOrder, (id) =>
			this.cancelJob(id),
		);

		this.updateStatus(this.model.currentStatus);
		this.updateConcurrencyIndicator();

		this.unlockWorkbench();
		this.coverArt.reset();
	}

	public get isCurrentlyProcessing(): boolean {
		return this.model.isProcessing;
	}

	public getCurrentStatus(): ProcessingStatus {
		return { ...this.model.currentStatus };
	}

	public handleQueueSnapshot(event: ProcessingQueueEvent): void {
		this.applyQueueSnapshot(event);
	}

	public updateProgress(event: ProcessingProgressEvent): void {
		this.applyProgress(event);
	}

	private buildJobKey(inputIndex?: number, jobId?: string): string {
		return buildJobKeyDomain(inputIndex, jobId);
	}

	private buildInferredProgressLabel(event: ProcessingProgressEvent): string {
		if (typeof event.input_index === 'number') {
			const path = this.findFilePathByIndex(event.input_index);
			if (path) {
				return buildQueueLabels([path])[0] ?? path;
			}
		}

		if (event.job_id) {
			return event.job_id.slice(0, 8);
		}

		if (event.current_file) {
			const filename = extractFilenameFromProgress(event.current_file);
			if (filename) return filename;
		}

		return 'Processing';
	}

	private cancellableForegroundJobIds(): string[] {
		return Array.from(
			new Set(
				Array.from(this.model.jobProgress.values())
					.filter((job) => job.status === 'processing' && job.jobId)
					.map((job) => job.jobId as string),
			),
		);
	}

	private handleIntents(intents: StatusPanelIntent[]): void {
		for (const intent of intents) {
			if (intent.kind === 'single-completion-hold') {
				this.scheduleSingleCompletion(intent);
			} else {
				this.scheduleBatchCompletion(intent.holdMs);
			}
		}
	}

	private scheduleBatchCompletion(holdMs: number): void {
		if (this.batchCompletionTimeout) return;

		this.batchCompletionTimeout = window.setTimeout(() => {
			this.batchCompletionTimeout = undefined;
			const result = completeBatchCompletionHold(this.model);
			this.model = result.model;
			this.applyIdleSideEffects();
			this.showCompletionFeedback(result.feedback);
		}, holdMs);
	}

	private scheduleSingleCompletion(
		intent: Extract<StatusPanelIntent, { kind: 'single-completion-hold' }>,
	): void {
		this.clearSingleCompletionTimeout();
		this.singleCompletionTimeout = window.setTimeout(() => {
			this.singleCompletionTimeout = undefined;
			const result = completeSingleCompletionHold(this.model, intent.jobKey, intent);
			this.model = result.model;
			if (result.feedback) {
				this.applyIdleSideEffects();
				this.showCompletionFeedback(result.feedback);
			} else {
				this.renderModel();
			}
		}, intent.holdMs);
	}

	private clearBatchCompletionTimeout(): void {
		if (this.batchCompletionTimeout) {
			window.clearTimeout(this.batchCompletionTimeout);
			this.batchCompletionTimeout = undefined;
		}
	}

	private clearSingleCompletionTimeout(): void {
		if (this.singleCompletionTimeout) {
			window.clearTimeout(this.singleCompletionTimeout);
			this.singleCompletionTimeout = undefined;
		}
	}

	private calculateAggregateProgressAndStage(): {
		aggregate: AggregateProgress;
		stage: ProcessingStatus['stage'];
	} {
		return calculateAggregateProgressAndStage(this.model.jobProgress);
	}

	private renderModel(): void {
		renderJobList(this.view, this.model.jobProgress, this.model.queueOrder, (id) =>
			this.cancelJob(id),
		);
		const { aggregate } = this.calculateAggregateProgressAndStage();
		this.updateConcurrencyIndicator(aggregate);
		this.updateStatus(this.model.currentStatus);
	}

	private updateConcurrencyIndicator(aggregate?: AggregateProgress): void {
		renderConcurrencyStatus(this.view, this.readConcurrency(), aggregate);
	}

	private updateStatus(status: ProcessingStatus): void {
		this.model = {
			...this.model,
			currentStatus: status,
		};
		renderStatus(this.view, status, this.model.isProcessing);
	}

	private applyIdleSideEffects(): void {
		this.progressSubscription.stop();
		this.clearBatchCompletionTimeout();
		this.clearSingleCompletionTimeout();
		this.pendingRender = false;
		renderJobList(this.view, this.model.jobProgress, this.model.queueOrder, (id) =>
			this.cancelJob(id),
		);
		this.updateStatus(this.model.currentStatus);
		this.updateConcurrencyIndicator();
		this.unlockWorkbench();
		this.coverArt.reset();
	}

	private showCompletionFeedback(feedbackResult: StatusPanelCompletionFeedback): void {
		if (feedbackResult.kind === 'success') {
			this.view.showSuccess(feedbackResult.message);
		} else if (feedbackResult.kind === 'error') {
			this.view.showError(feedbackResult.message);
		} else {
			this.view.showInfo(feedbackResult.message);
		}
	}

	private cancelJob(_jobId: string): void {
		// Per-row cancel: the foreground/direct lane has no backend cancel command
		// (operation-scoped cancel lives in the Work Center). Settle the local
		// foreground render; any in-flight preview completes and auto-opens
		// normally. Preview is an ephemeral render lane, never a Work Center row.
		this.handleProcessingCancellation();
	}

	private scheduleRender(immediate: boolean): void {
		if (immediate) {
			this.flushRender();
		} else if (!this.pendingRender) {
			this.pendingRender = true;
			requestAnimationFrame(() => this.flushRender());
		}
	}

	private flushRender(): void {
		this.pendingRender = false;

		renderJobList(this.view, this.model.jobProgress, this.model.queueOrder, (id) =>
			this.cancelJob(id),
		);
		const { aggregate } = this.calculateAggregateProgressAndStage();
		this.updateConcurrencyIndicator(aggregate);
		this.updateStatus(this.model.currentStatus);

		if (this.model.currentWorkKind === 'batch' && this.model.latestProgressEvent) {
			const event = this.model.latestProgressEvent;
			const indexedPath =
				typeof event.input_index === 'number' ? this.findFilePathByIndex(event.input_index) : null;
			if (indexedPath) {
				void this.coverArt.syncForFile(indexedPath);
			} else if (event.current_file) {
				const filePath = this.findFilePathByCurrentFile(event.current_file);
				if (filePath) {
					void this.coverArt.syncForFile(filePath);
				}
			}
		}
	}

	private findFilePathByCurrentFile(currentFile: string): string | null {
		return findFilePathByCurrentFileService(this.readValidTitles(), currentFile);
	}

	private findFilePathByIndex(index: number): string | null {
		return findFilePathByIndexService(this.readValidTitles(), index);
	}
}
