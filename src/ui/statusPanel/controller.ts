import type { MetadataSaveBatchResult } from '../../types/metadata';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../types/events';
import { getCurrentFileList, setFileOrderLocked } from '../fileList';
import { setJobControlsEnabled } from '../jobControls';
import { buildQueueLabels, extractFilenameFromProgress } from './formatting';
import { startProcessing as startProcessingAction } from './processing';
import { renderConcurrencyStatus, renderJobList, renderStatus } from './render';
import { buildStatus, type AggregateProgress, type ProcessingStatus } from './state';
import { buildMetadataSaveCompletionFeedback } from './metadataSaveFeedback';
import type { ProcessCommandResult } from '../../types/audio';
import { calculateAggregateProgressAndStage } from './domain/aggregate';
import { buildJobKey as buildJobKeyDomain } from './domain/jobKeys';
import { createCoverArtTracker } from './services/coverArtTracker';
import {
	findFilePathByIndex as findFilePathByIndexService,
	findFilePathByCurrentFile as findFilePathByCurrentFileService,
} from './services/fileLookup';
import { createProgressSubscription } from './services/progressSubscription';
import {
	enterCancelAllCancellationWorkflow,
	liveProcessingCancellationWorkflowServices,
	ProcessingCancellationWorkflowLive,
	runProcessingCancellationWorkflow,
} from './processingCancellationWorkflow';
import {
	applyCancellation,
	applyProgress,
	applyQueueSnapshot,
	completeBatchCompletionHold,
	completeMergeSkipHold,
	completeSingleCompletionHold,
	createStatusPanelModel,
	isTerminalProgressStage,
	reconcileProcessResult,
	resetStatusPanelModel,
	withBatchCompletionMessage,
	withCurrentWorkKind,
	type StatusPanelCompletionFeedback,
	type StatusPanelIntent,
	type StatusPanelModel,
	workKindFromOperationKind,
} from './domain/stateMachine';
import { pushTransientStatusMessage, showError, showInfo, showSuccess } from './viewState.svelte';

export class StatusPanelRuntime {
	private readonly progressSubscription = createProgressSubscription({
		onProgress: (event) => this.updateProgress(event),
		onQueue: (event) => this.handleQueueSnapshot(event),
	});
	private readonly coverArt = createCoverArtTracker();
	private model: StatusPanelModel;
	private batchCompletionTimeout?: number;
	private singleCompletionTimeout?: number;
	private pendingRender = false;

	constructor() {
		this.model = createStatusPanelModel();
		this.renderModel();
		this.updateConcurrencyIndicator();
		this.coverArt.reset();
	}

	public async startProcessing(options?: { previewSeconds?: number }): Promise<void> {
		this.clearSingleCompletionTimeout();
		this.clearBatchCompletionTimeout();

		return startProcessingAction(
			{
				updateStatus: (status) => this.updateStatus(status),
				setProcessingState: (isProcessing) => {
					this.model = {
						...this.model,
						isProcessing,
					};
				},
				updateArtThumbnail: () => this.coverArt.syncForCurrentList(),
				startProgressListener: () => this.progressSubscription.start(),
				setCurrentWorkKind: (workKind) => {
					this.model = withCurrentWorkKind(this.model, workKind);
				},
				setBatchCompletionMessage: (message) => this.setBatchCompletionMessage(message),
				reconcileProcessResult: (result) => this.reconcileProcessResult(result),
				handleCancellation: () => this.handleProcessingCancellation(),
				resetToIdle: () => this.resetToIdle(),
			},
			options,
		);
	}

	public setBatchCompletionMessage(message: string | null): void {
		this.model = withBatchCompletionMessage(this.model, message);
	}

	public async beginMetadataSave(): Promise<void> {
		this.clearSingleCompletionTimeout();
		this.clearBatchCompletionTimeout();
		this.model = withCurrentWorkKind(
			{
				...this.model,
				isProcessing: true,
				batchCompletionMessageOverride: null,
			},
			'metadataSave',
		);
		this.updateStatus(buildStatus('writing', 0, 'Preparing metadata save...'));
		setJobControlsEnabled(false);
		setFileOrderLocked(true);
		await this.progressSubscription.start();
		this.renderModel();
	}

	public completeMetadataSave(result: MetadataSaveBatchResult): void {
		const feedbackResult = buildMetadataSaveCompletionFeedback(result);
		this.setBatchCompletionMessage(feedbackResult.message);
		if (this.model.jobProgress.size === 0) {
			this.model = resetStatusPanelModel();
			this.applyIdleSideEffects();
			this.showCompletionFeedback(feedbackResult);
		}
	}

	public failMetadataSave(message = 'Save failed - see console'): void {
		const feedbackResult: StatusPanelCompletionFeedback = { kind: 'error', message };
		this.model = resetStatusPanelModel();
		this.applyIdleSideEffects();
		this.showCompletionFeedback(feedbackResult);
	}

	public reconcileProcessResult(result: ProcessCommandResult): void {
		const mergeOutputLabel = this.buildMergeOutputLabel();
		const transition = reconcileProcessResult(this.model, result, Date.now(), {
			mergeOutputLabel,
		});
		this.model = transition.model;
		this.renderModel();
		this.handleIntents(transition.intents);
	}

	public applyQueueSnapshot(event: ProcessingQueueEvent): void {
		this.clearBatchCompletionTimeout();
		this.clearSingleCompletionTimeout();

		const transition = applyQueueSnapshot(this.model, event, Date.now());
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

	public async requestCancelAll(): Promise<void> {
		const jobIds = this.cancellableForegroundJobIds();
		if (jobIds.length === 0) {
			return;
		}
		const preparedCancelAll = enterCancelAllCancellationWorkflow(
			liveProcessingCancellationWorkflowServices,
			jobIds,
		);
		await runProcessingCancellationWorkflow(
			{
				type: 'cancelAll',
				jobIds,
				getCurrentStatus: () => this.model.currentStatus,
				updateStatus: (status) => this.updateStatus(status),
			},
			ProcessingCancellationWorkflowLive,
			preparedCancelAll,
		);
	}

	public handleProcessingCancellation(): void {
		if (this.batchCompletionTimeout || this.singleCompletionTimeout) {
			return;
		}

		if (this.model.jobProgress.size === 0) {
			showInfo('Processing was cancelled.');
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
		renderJobList(this.model.jobProgress, this.model.queueOrder, (id) => this.cancelJob(id));

		this.updateStatus(this.model.currentStatus);
		this.updateConcurrencyIndicator();

		setJobControlsEnabled(true);
		setFileOrderLocked(false);
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
		const fileList = getCurrentFileList();
		const workKind = workKindFromOperationKind(event.operation_kind);
		if (workKind === 'merge' && fileList?.files?.length) {
			const firstValidFile = fileList.files.find((file) => file.isValid);
			if (firstValidFile?.path) {
				return buildQueueLabels([firstValidFile.path])[0] ?? firstValidFile.path;
			}
		}
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

	private buildMergeOutputLabel(): string {
		const fileList = getCurrentFileList();
		const firstValidPath = fileList?.files.find((file) => file.isValid)?.path;
		return firstValidPath
			? (buildQueueLabels([firstValidPath])[0] ?? firstValidPath)
			: 'Merge output';
	}

	private handleIntents(intents: StatusPanelIntent[]): void {
		for (const intent of intents) {
			if (intent.kind === 'single-completion-hold') {
				this.scheduleSingleCompletion(intent);
			} else if (intent.kind === 'batch-completion-hold') {
				this.scheduleBatchCompletion(intent.holdMs);
			} else {
				this.scheduleMergeSkipCompletion(intent.jobKey, intent.message, intent.holdMs);
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

	private scheduleMergeSkipCompletion(jobKey: string, message: string, holdMs: number): void {
		this.clearSingleCompletionTimeout();
		this.singleCompletionTimeout = window.setTimeout(() => {
			this.singleCompletionTimeout = undefined;
			const result = completeMergeSkipHold(this.model, jobKey, message);
			this.model = result.model;
			if (result.feedback) {
				this.applyIdleSideEffects();
				this.showCompletionFeedback(result.feedback);
			} else {
				this.renderModel();
			}
		}, holdMs);
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
		renderJobList(this.model.jobProgress, this.model.queueOrder, (id) => this.cancelJob(id));
		const { aggregate } = this.calculateAggregateProgressAndStage();
		this.updateConcurrencyIndicator(aggregate);
		this.updateStatus(this.model.currentStatus);
	}

	private updateConcurrencyIndicator(aggregate?: AggregateProgress): void {
		renderConcurrencyStatus(aggregate);
	}

	private updateStatus(status: ProcessingStatus): void {
		this.model = {
			...this.model,
			currentStatus: status,
		};
		renderStatus(status, this.model.isProcessing);
	}

	private applyIdleSideEffects(): void {
		this.progressSubscription.stop();
		this.clearBatchCompletionTimeout();
		this.clearSingleCompletionTimeout();
		this.pendingRender = false;
		renderJobList(this.model.jobProgress, this.model.queueOrder, (id) => this.cancelJob(id));
		this.updateStatus(this.model.currentStatus);
		this.updateConcurrencyIndicator();
		setJobControlsEnabled(true);
		setFileOrderLocked(false);
		this.coverArt.reset();
	}

	private showCompletionFeedback(feedbackResult: StatusPanelCompletionFeedback): void {
		if (feedbackResult.kind === 'success') {
			showSuccess(feedbackResult.message);
		} else if (feedbackResult.kind === 'error') {
			showError(feedbackResult.message);
		} else {
			showInfo(feedbackResult.message);
		}
	}

	private async cancelJob(jobId: string): Promise<void> {
		await runProcessingCancellationWorkflow(
			{ type: 'cancelJob', jobId },
			ProcessingCancellationWorkflowLive,
		);
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

		renderJobList(this.model.jobProgress, this.model.queueOrder, (id) => this.cancelJob(id));
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
		return findFilePathByCurrentFileService(getCurrentFileList(), currentFile);
	}

	private findFilePathByIndex(index: number): string | null {
		return findFilePathByIndexService(getCurrentFileList(), index);
	}
}

let statusPanelInstance: StatusPanelRuntime | null = null;

export function initStatusPanel(): StatusPanelRuntime {
	if (!statusPanelInstance) {
		statusPanelInstance = new StatusPanelRuntime();
	}
	return statusPanelInstance;
}
export function isStatusPanelProcessing(): boolean {
	return Boolean(statusPanelInstance?.isCurrentlyProcessing);
}
export function triggerProcessFromStatusPanel(options?: { previewSeconds?: number }): void {
	void initStatusPanel().startProcessing(options);
}
export function triggerCancelAllFromStatusPanel(): void {
	if (!statusPanelInstance?.isCurrentlyProcessing) return;
	void statusPanelInstance?.requestCancelAll();
}
export function beginMetadataSaveInStatusPanel(): Promise<void> {
	return initStatusPanel().beginMetadataSave();
}
export function completeMetadataSaveInStatusPanel(result: MetadataSaveBatchResult): void {
	initStatusPanel().completeMetadataSave(result);
}
export function failMetadataSaveInStatusPanel(message?: string): void {
	initStatusPanel().failMetadataSave(message);
}
export function pushStatusPanelTransientStatus(
	message: string,
	options?: { ttlMs?: number },
): void {
	pushTransientStatusMessage(message, options?.ttlMs);
}
