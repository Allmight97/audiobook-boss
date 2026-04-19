import { tauriClient } from '../../lib/tauri/client';
import { STAGES } from '../../types/events';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../types/events';
import { publishQueueMirror } from '../core/appStore.svelte';
import { getCurrentFileList, setFileOrderLocked } from '../fileList';
import { setJobControlsEnabled } from '../jobControls';
import * as dom from './dom';
import { listenForProgressEvents, listenForQueueEvents } from './events';
import {
	buildQueueLabels,
	extractFilenameFromProgress,
	formatAggregateMessage,
} from './formatting';
import { startProcessing as startProcessingAction } from './processing';
import { renderConcurrencyStatus, renderJobList, renderStatus } from './render';
import {
	buildStatus,
	createInitialStatus,
	type AggregateProgress,
	type JobProgress,
	type JobStatus,
	type ProcessingStatus,
} from './state';
import type { ProcessCommandResult } from '../../types/audio';
import { calculateAggregateProgressAndStage as calculateAggregateProgressAndStageDomain } from './domain/aggregate';
import { buildJobKey as buildJobKeyDomain } from './domain/jobKeys';
import { areAllBatchJobsTerminal, buildQueueSnapshotState } from './domain/queueState';
import { readCoverArtDataUrl, shouldSkipCoverArtRead } from './services/artThumbnail';
import {
	findFilePathByIndex as findFilePathByIndexService,
	findFilePathByCurrentFile as findFilePathByCurrentFileService,
} from './services/fileLookup';
import { isTerminalProgressEvent, shouldThrottleProgressUpdate } from './services/progressThrottle';

function toTerminalJobStatus(
	stage: ProcessingProgressEvent['stage'],
): Extract<JobStatus, 'completed' | 'failed' | 'cancelled'> {
	if (stage === STAGES.failed) return 'failed';
	if (stage === STAGES.cancelled) return 'cancelled';
	return 'completed';
}

export class StatusPanelController {
	private progressUnlisten?: () => void;
	private queueUnlisten?: () => void;
	private isProcessing = false;
	private currentStatus: ProcessingStatus;
	private jobProgress: Map<string, JobProgress> = new Map();
	private queueOrder: string[] = [];
	private queueOrderSet: Set<string> = new Set();
	private lastProgressRenderByKey: Map<string, number> = new Map();
	private batchCompletionTimeout?: number;
	private singleCompletionTimeout?: number;
	private batchCompletionMessageOverride: string | null = null;
	private currentJobType: 'merge' | 'batch' | null = null;
	private lastCoverArtPath: string | null = null;
	private pendingRender = false;
	private latestProgressEvent: ProcessingProgressEvent | null = null;

	constructor() {
		this.currentStatus = createInitialStatus();
		this.updateUI();
		this.updateConcurrencyIndicator();
		dom.resetArtThumbnail();

		window.addEventListener('beforeunload', () => {
			this.teardownEventListeners();
		});
	}

	public async startProcessing(options?: { previewSeconds?: number }): Promise<void> {
		this.clearSingleCompletionTimeout();
		this.clearBatchCompletionTimeout();

		return startProcessingAction(
			{
				updateStatus: (status) => this.updateStatus(status),
				setProcessingState: (isProcessing) => {
					this.isProcessing = isProcessing;
				},
				updateArtThumbnail: () => this.updateArtThumbnail(),
				startProgressListener: () => this.startEventListeners(),
				setCurrentJobType: (jobType) => {
					this.currentJobType = jobType;
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
		this.batchCompletionMessageOverride = message;
	}

	public reconcileProcessResult(result: ProcessCommandResult): void {
		if (result.jobType === 'merge') {
			const skippedMergeEntry = result.results.find((entry) => entry.status === 'skipped');
			if (skippedMergeEntry) {
				this.reconcileMergeSkip(skippedMergeEntry);
				return;
			}
		}

		let updated = false;
		for (const entry of result.results) {
			if (entry.status !== 'skipped' || typeof entry.inputIndex !== 'number') {
				continue;
			}

			const key = this.buildJobKey(entry.inputIndex, undefined);
			const existing = this.jobProgress.get(key);
			this.jobProgress.set(key, {
				jobId: existing?.jobId,
				inputIndex: entry.inputIndex,
				label:
					existing?.label ??
					this.findFilePathByIndex(entry.inputIndex) ??
					`Input ${entry.inputIndex + 1}`,
				status: 'skipped',
				stage: existing?.stage,
				percentage: 100,
				message: entry.message,
				lastUpdate: Date.now(),
			});
			updated = true;
		}

		if (!updated) {
			return;
		}

		this.updateAggregateUI();
		renderJobList(this.jobProgress, this.queueOrder, (id) => this.cancelJob(id));
		if (areAllBatchJobsTerminal(this.queueOrder, this.jobProgress)) {
			this.scheduleBatchCompletion();
		}
	}

	private reconcileMergeSkip(entry: ProcessCommandResult['results'][number]): void {
		const now = Date.now();
		const key = this.buildJobKey(undefined, entry.jobId ?? undefined);
		const fileList = getCurrentFileList();
		const firstValidPath = fileList?.files.find((file) => file.isValid)?.path;
		const label = firstValidPath
			? (buildQueueLabels([firstValidPath])[0] ?? firstValidPath)
			: 'Merge output';

		this.jobProgress.set(key, {
			jobId: entry.jobId ?? undefined,
			inputIndex: undefined,
			label,
			status: 'skipped',
			percentage: 100,
			message: entry.message,
			lastUpdate: now,
		});
		this.isProcessing = true;
		this.updateAggregateUI();
		this.scheduleMergeSkipCompletion(key, entry.message);
	}

	public async startEventListeners(): Promise<void> {
		this.teardownEventListeners();

		this.progressUnlisten = await listenForProgressEvents((progress) => {
			this.updateProgress(progress);
		});

		this.queueUnlisten = await listenForQueueEvents((queue) => {
			this.handleQueueSnapshot(queue);
		});
	}

	public applyQueueSnapshot(event: ProcessingQueueEvent): void {
		const now = Date.now();
		const queueSnapshotState = buildQueueSnapshotState(event.items, now);

		this.clearBatchCompletionTimeout();
		this.clearSingleCompletionTimeout();

		this.jobProgress.clear();
		this.queueOrder = queueSnapshotState.queueOrder;
		this.queueOrderSet = new Set(queueSnapshotState.queueOrder);
		this.lastProgressRenderByKey.clear();

		queueSnapshotState.jobProgress.forEach((job, key) => {
			this.jobProgress.set(key, job);
		});

		this.isProcessing = this.jobProgress.size > 0;
		const { aggregate, stage } = this.calculateAggregateProgressAndStage();
		this.updateConcurrencyIndicator(aggregate);
		this.updateStatus(
			buildStatus(
				stage,
				aggregate.overallPercentage,
				formatAggregateMessage(this.jobProgress, aggregate),
			),
		);
		renderJobList(this.jobProgress, this.queueOrder, (id) => this.cancelJob(id));
	}

	public applyProgress(event: ProcessingProgressEvent): void {
		const jobKey = this.buildJobKey(event.input_index, event.job_id ?? undefined);
		const now = Date.now();
		const isTerminal = isTerminalProgressEvent(event);
		const lastRender = this.lastProgressRenderByKey.get(jobKey) ?? 0;
		if (shouldThrottleProgressUpdate(now, lastRender, isTerminal)) {
			return;
		}
		this.lastProgressRenderByKey.set(jobKey, now);

		this.latestProgressEvent = event;

		const existing = this.jobProgress.get(jobKey);
		const jobStatus: JobStatus = isTerminal ? toTerminalJobStatus(event.stage) : 'processing';

		this.jobProgress.set(jobKey, {
			jobId: event.job_id ?? existing?.jobId,
			inputIndex: typeof event.input_index === 'number' ? event.input_index : existing?.inputIndex,
			label: existing?.label ?? this.buildFallbackLabel(event),
			status: jobStatus,
			stage: event.stage,
			percentage: Math.round(event.percentage * 10) / 10,
			message: event.message,
			lastUpdate: now,
		});

		if (typeof event.input_index === 'number') {
			const indexedKey = this.buildJobKey(event.input_index, undefined);
			if (!this.queueOrderSet.has(indexedKey)) {
				this.queueOrder.push(indexedKey);
				this.queueOrderSet.add(indexedKey);
			}
		}

		this.isProcessing = this.jobProgress.size > 0;

		const isBatchActive = this.queueOrder.length > 0;
		if (isTerminal) {
			if (!isBatchActive) {
				this.scheduleSingleCompletion(jobKey, { stage: event.stage, message: event.message });
			} else if (this.areAllBatchJobsTerminal()) {
				this.scheduleBatchCompletion();
			}
		}

		this.scheduleRender(isTerminal);
	}

	public async requestCancelAll(): Promise<void> {
		dom.setCancelAllButtonPending(true);
		try {
			await tauriClient.cancelProcessing();
			this.updateStatus(
				buildStatus(
					this.currentStatus.stage,
					this.currentStatus.percentage,
					'Cancellation requested…',
				),
			);
		} catch (error) {
			console.error('Failed to cancel processing:', error);
			dom.showError('Failed to cancel processing. Please try again.');
		} finally {
			dom.setCancelAllButtonPending(false);
		}
	}

	public handleProcessingCancellation(): void {
		if (this.batchCompletionTimeout || this.singleCompletionTimeout) {
			return;
		}

		if (this.jobProgress.size === 0) {
			dom.showInfo('Processing was cancelled.');
			this.resetToIdle();
			return;
		}

		const now = Date.now();
		for (const [jobKey, job] of this.jobProgress.entries()) {
			if (job.status === 'completed' || job.status === 'failed' || job.status === 'cancelled') {
				continue;
			}
			this.jobProgress.set(jobKey, {
				...job,
				status: 'cancelled',
				stage: STAGES.cancelled,
				message: 'Processing was cancelled.',
				lastUpdate: now,
			});
		}

		if (this.queueOrder.length > 0) {
			this.scheduleBatchCompletion();
		} else {
			const [jobKey] = this.jobProgress.keys();
			if (!jobKey) {
				dom.showInfo('Processing was cancelled.');
				this.resetToIdle();
				return;
			}
			this.scheduleSingleCompletion(jobKey, {
				stage: STAGES.cancelled,
				message: 'Processing was cancelled.',
			});
		}

		this.scheduleRender(true);
	}

	public resetToIdle(): void {
		this.isProcessing = false;
		this.batchCompletionMessageOverride = null;
		this.currentJobType = null;
		this.lastCoverArtPath = null;

		this.teardownEventListeners();
		this.clearBatchCompletionTimeout();
		this.clearSingleCompletionTimeout();

		this.jobProgress.clear();
		this.queueOrder = [];
		this.queueOrderSet.clear();
		this.lastProgressRenderByKey.clear();
		this.pendingRender = false;
		this.latestProgressEvent = null;
		renderJobList(this.jobProgress, this.queueOrder, (id) => this.cancelJob(id));

		this.updateStatus(createInitialStatus());
		this.updateConcurrencyIndicator();

		setJobControlsEnabled(true);
		setFileOrderLocked(false);
		dom.resetArtThumbnail();
	}

	public get isCurrentlyProcessing(): boolean {
		return this.isProcessing;
	}

	public getCurrentStatus(): ProcessingStatus {
		return { ...this.currentStatus };
	}

	public handleQueueSnapshot(event: ProcessingQueueEvent): void {
		this.applyQueueSnapshot(event);
	}

	public updateProgress(event: ProcessingProgressEvent): void {
		this.applyProgress(event);
	}

	private teardownEventListeners(): void {
		if (this.progressUnlisten) {
			this.progressUnlisten();
			this.progressUnlisten = undefined;
		}
		if (this.queueUnlisten) {
			this.queueUnlisten();
			this.queueUnlisten = undefined;
		}
	}

	private buildJobKey(inputIndex?: number, jobId?: string): string {
		return buildJobKeyDomain(inputIndex, jobId);
	}

	private buildFallbackLabel(event: ProcessingProgressEvent): string {
		const fileList = getCurrentFileList();
		if (this.currentJobType === 'merge' && fileList?.files?.length) {
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

	private scheduleBatchCompletion(): void {
		if (this.batchCompletionTimeout) return;

		this.batchCompletionTimeout = window.setTimeout(() => {
			this.batchCompletionTimeout = undefined;

			const hasFailed = Array.from(this.jobProgress.values()).some(
				(job) => job.status === 'failed',
			);
			const hasCancelled = Array.from(this.jobProgress.values()).some(
				(job) => job.status === 'cancelled',
			);

			if (hasFailed) {
				dom.showError(
					this.batchCompletionMessageOverride ?? 'One or more files failed to process.',
				);
			} else if (hasCancelled) {
				dom.showInfo('Processing was cancelled.');
			} else if (this.batchCompletionMessageOverride) {
				dom.showSuccess(this.batchCompletionMessageOverride);
			} else {
				dom.showSuccess('Audiobook created successfully!');
			}

			this.resetToIdle();
		}, 2000);
	}

	private scheduleSingleCompletion(
		jobKey: string,
		event: Pick<ProcessingProgressEvent, 'stage' | 'message'>,
	): void {
		this.clearSingleCompletionTimeout();
		this.singleCompletionTimeout = window.setTimeout(() => {
			this.singleCompletionTimeout = undefined;
			this.jobProgress.delete(jobKey);
			if (this.jobProgress.size === 0) {
				this.resetToIdle();

				if (event.stage === STAGES.completed) {
					dom.showSuccess('Audiobook created successfully!');
				} else if (event.stage === STAGES.failed) {
					dom.showError(event.message);
				} else if (event.stage === STAGES.cancelled) {
					dom.showInfo('Processing was cancelled.');
				}
			}

			this.updateAggregateUI();
			renderJobList(this.jobProgress, this.queueOrder, (id) => this.cancelJob(id));
		}, 2000);
	}

	private scheduleMergeSkipCompletion(jobKey: string, message: string): void {
		this.clearSingleCompletionTimeout();
		this.singleCompletionTimeout = window.setTimeout(() => {
			this.singleCompletionTimeout = undefined;
			this.jobProgress.delete(jobKey);
			if (this.jobProgress.size === 0) {
				this.resetToIdle();
				dom.showInfo(message);
				return;
			}

			this.updateAggregateUI();
			renderJobList(this.jobProgress, this.queueOrder, (id) => this.cancelJob(id));
		}, 1500);
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

	private areAllBatchJobsTerminal(): boolean {
		return areAllBatchJobsTerminal(this.queueOrder, this.jobProgress);
	}

	private calculateAggregateProgressAndStage(): {
		aggregate: AggregateProgress;
		stage: ProcessingStatus['stage'];
	} {
		return calculateAggregateProgressAndStageDomain(this.jobProgress);
	}

	private updateAggregateUI(): void {
		if (this.jobProgress.size === 0 && !this.isProcessing) {
			return;
		}
		this.isProcessing = this.jobProgress.size > 0;
		const { aggregate, stage } = this.calculateAggregateProgressAndStage();
		const status = buildStatus(
			stage,
			aggregate.overallPercentage,
			formatAggregateMessage(this.jobProgress, aggregate),
		);
		this.updateStatus(status);
		this.updateConcurrencyIndicator(aggregate);
		renderJobList(this.jobProgress, this.queueOrder, (id) => this.cancelJob(id));
	}

	private updateConcurrencyIndicator(aggregate?: AggregateProgress): void {
		renderConcurrencyStatus(aggregate);
	}

	private updateStatus(status: ProcessingStatus): void {
		this.currentStatus = status;
		this.updateUI();
		publishQueueMirror({
			summary: status.message,
			statusText: String(status.stage),
		});
	}

	private updateUI(): void {
		renderStatus(this.currentStatus, this.isProcessing);
	}

	private async cancelJob(jobId: string): Promise<void> {
		try {
			await tauriClient.cancelProcessing(jobId);
		} catch (error) {
			console.error(`Failed to cancel job ${jobId}:`, error);
			dom.showError(`Failed to cancel job ${jobId}`);
		}
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

		renderJobList(this.jobProgress, this.queueOrder, (id) => this.cancelJob(id));

		const { aggregate, stage } = this.calculateAggregateProgressAndStage();
		this.updateConcurrencyIndicator(aggregate);

		const status = buildStatus(
			stage,
			aggregate.overallPercentage,
			formatAggregateMessage(this.jobProgress, aggregate),
			{
				currentFile: this.latestProgressEvent?.current_file,
				etaSeconds: this.latestProgressEvent?.eta_seconds,
			},
		);
		this.updateStatus(status);

		if (this.currentJobType === 'batch' && this.latestProgressEvent) {
			const event = this.latestProgressEvent;
			const indexedPath =
				typeof event.input_index === 'number' ? this.findFilePathByIndex(event.input_index) : null;
			if (indexedPath) {
				void this.updateArtThumbnailForFile(indexedPath);
			} else if (event.current_file) {
				const filePath = this.findFilePathByCurrentFile(event.current_file);
				if (filePath) {
					void this.updateArtThumbnailForFile(filePath);
				}
			}
		}
	}

	private async updateArtThumbnail(): Promise<void> {
		const fileList = getCurrentFileList();
		if (!fileList || !fileList.files.length) {
			dom.resetArtThumbnail();
			return;
		}

		const firstValidFile = fileList.files.find((file) => file.isValid);
		if (!firstValidFile) {
			dom.resetArtThumbnail();
			return;
		}

		await this.updateArtThumbnailForFile(firstValidFile.path);
	}

	private async updateArtThumbnailForFile(filePath: string): Promise<void> {
		if (shouldSkipCoverArtRead(this.lastCoverArtPath, filePath)) {
			return;
		}
		this.lastCoverArtPath = filePath;

		try {
			const dataUrl = await readCoverArtDataUrl(filePath);
			if (dataUrl) {
				dom.displayCoverArt(dataUrl);
			} else {
				dom.resetArtThumbnail();
			}
		} catch (error) {
			console.warn('Failed to load cover art for thumbnail:', error);
			dom.resetArtThumbnail();
		}
	}

	private findFilePathByCurrentFile(currentFile: string): string | null {
		return findFilePathByCurrentFileService(getCurrentFileList(), currentFile);
	}

	private findFilePathByIndex(index: number): string | null {
		return findFilePathByIndexService(getCurrentFileList(), index);
	}
}
