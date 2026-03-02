/**
 * StatusPanel business logic and state management
 *
 * This module contains the core StatusPanel class with event handling,
 * processing coordination, and state management.
 */

import { tauriClient } from '../../lib/tauri/client';
import { STAGES } from '../../types/events';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../types/events';
import { publishQueueMirror } from '../core/appStore.svelte';
import { getCurrentFileList, setFileOrderLocked } from '../fileList';
import * as dom from './dom';
import { setJobControlsEnabled } from '../jobControls';
import { listenForProgressEvents, listenForQueueEvents } from './events';
import {
	buildQueueLabels,
	extractFilenameFromProgress,
	formatAggregateMessage,
} from './formatting';
import { startProcessing as startProcessingAction } from './processing';
import { renderConcurrencyStatus, renderJobList, renderStatus } from './render';
import {
	createInitialStatus,
	type AggregateProgress,
	type JobProgress,
	type JobStatus,
	type ProcessingStatus,
} from './state';
import { calculateAggregateProgressAndStage as calculateAggregateProgressAndStageDomain } from './domain/aggregate';
import { buildJobKey as buildJobKeyDomain } from './domain/jobKeys';
import { areAllBatchJobsTerminal, buildQueueSnapshotState } from './domain/queueState';
import { readCoverArtDataUrl, shouldSkipCoverArtRead } from './services/artThumbnail';
import {
	findFilePathByIndex as findFilePathByIndexService,
	findFilePathByName as findFilePathByNameService,
} from './services/fileLookup';
import { isTerminalProgressEvent, shouldThrottleProgressUpdate } from './services/progressThrottle';

export class StatusPanel {
	private progressUnlisten?: () => void;
	private queueUnlisten?: () => void;
	private isProcessing: boolean = false;
	private currentStatus: ProcessingStatus;
	/** Per-job progress tracking for parallel batch processing */
	private jobProgress: Map<string, JobProgress> = new Map();
	private queueOrder: string[] = [];
	private queueOrderSet: Set<string> = new Set();
	private lastProgressRenderByKey: Map<string, number> = new Map();
	private batchCompletionTimeout?: number;
	private singleCompletionTimeout?: number;
	private batchCompletionMessageOverride: string | null = null;
	private currentJobType: 'merge' | 'batch' | null = null;
	private lastCoverArtPath: string | null = null;
	private onMaxConcurrentUpdated: () => void;
	/** Track pending render to coalesce rAF batches */
	private pendingRender = false;
	/** Latest progress event data for deferred rendering */
	private latestProgressEvent: ProcessingProgressEvent | null = null;

	constructor() {
		this.currentStatus = createInitialStatus();
		this.onMaxConcurrentUpdated = () => this.updateConcurrencyIndicator();
		this.updateUI();
		this.updateConcurrencyIndicator();
		dom.resetArtThumbnail();
		// initialized in main.ts now: this.initializeMaxConcurrentControl();
		document.addEventListener('abb:max-concurrent-updated', this.onMaxConcurrentUpdated);

		// Ensure event listeners are cleaned up if the window unloads
		window.addEventListener('beforeunload', () => {
			if (this.progressUnlisten) {
				this.progressUnlisten();
				this.progressUnlisten = undefined;
			}
			if (this.queueUnlisten) {
				this.queueUnlisten();
				this.queueUnlisten = undefined;
			}
			document.removeEventListener('abb:max-concurrent-updated', this.onMaxConcurrentUpdated);
		});
	}

	// MaxConcurrent control moved to src/ui/jobControls.ts

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
				startProgressListener: () => this.startProgressListener(),
				setCurrentJobType: (jobType) => {
					this.currentJobType = jobType;
				},
				setBatchCompletionMessage: (message) => this.setBatchCompletionMessage(message),
				resetToIdle: () => this.resetToIdle(),
			},
			options,
		);
	}

	public setBatchCompletionMessage(message: string | null): void {
		this.batchCompletionMessageOverride = message;
	}

	private async startProgressListener(): Promise<void> {
		if (this.progressUnlisten) {
			this.progressUnlisten();
		}
		if (this.queueUnlisten) {
			this.queueUnlisten();
		}

		this.progressUnlisten = await listenForProgressEvents((progress) => {
			this.updateProgress(progress);
		});

		this.queueUnlisten = await listenForQueueEvents((queue) => {
			this.handleQueueSnapshot(queue);
		});
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

	private handleQueueSnapshot(event: ProcessingQueueEvent): void {
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
		this.updateStatus({
			stage,
			percentage: aggregate.overallPercentage,
			message: formatAggregateMessage(this.jobProgress, aggregate),
		});
		renderJobList(this.jobProgress, this.queueOrder, (id) => this.cancelJob(id));
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

	public updateProgress(event: ProcessingProgressEvent): void {
		const jobKey = this.buildJobKey(event.input_index, event.job_id ?? undefined);
		const now = Date.now();

		// Throttle non-terminal updates to avoid UI flooding with many jobs
		const isTerminal = isTerminalProgressEvent(event);
		const lastRender = this.lastProgressRenderByKey.get(jobKey) ?? 0;
		if (shouldThrottleProgressUpdate(now, lastRender, isTerminal)) {
			return;
		}
		this.lastProgressRenderByKey.set(jobKey, now);

		// --- SYNCHRONOUS STATE UPDATES ---
		// Store latest event for deferred rendering
		this.latestProgressEvent = event;

		const existing = this.jobProgress.get(jobKey);
		const jobStatus: JobStatus = isTerminal ? (event.stage as JobStatus) : 'processing';

		// Update job progress map
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

		// Update queue order if new job
		if (typeof event.input_index === 'number') {
			const key = this.buildJobKey(event.input_index, undefined);
			if (!this.queueOrderSet.has(key)) {
				this.queueOrder.push(key);
				this.queueOrderSet.add(key);
			}
		}

		this.isProcessing = this.jobProgress.size > 0;

		const isBatchActive = this.queueOrder.length > 0;

		// Handle terminal events (completion/failure/cancellation)
		if (isTerminal) {
			if (!isBatchActive) {
				// Single-job mode: schedule cleanup and reset
				this.scheduleSingleCompletion(jobKey, { stage: event.stage, message: event.message });
			} else if (this.areAllBatchJobsTerminal()) {
				// Batch mode: all jobs terminal, schedule batch completion
				this.scheduleBatchCompletion();
			}
		}

		// --- DEFERRED UI RENDERING ---
		// Schedule render (immediate for terminal, batched for progress updates)
		this.scheduleRender(isTerminal);
	}

	private calculateAggregateProgressAndStage(): {
		aggregate: AggregateProgress;
		stage: ProcessingStatus['stage'];
	} {
		return calculateAggregateProgressAndStageDomain(this.jobProgress);
	}

	/** Update UI with aggregate progress (called after job removal) */
	private updateAggregateUI(): void {
		if (this.jobProgress.size === 0 && !this.isProcessing) {
			return; // No need to update if idle
		}
		this.isProcessing = this.jobProgress.size > 0;
		const { aggregate, stage } = this.calculateAggregateProgressAndStage();
		const status: ProcessingStatus = {
			stage,
			percentage: aggregate.overallPercentage,
			message: formatAggregateMessage(this.jobProgress, aggregate),
		};
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

	private async handleCancelAll(): Promise<void> {
		dom.setCancelAllButtonPending(true);
		try {
			await tauriClient.cancelProcessing();
			// Do not set final cancelled state here; wait for backend events
			this.updateStatus({
				stage: this.currentStatus.stage,
				percentage: this.currentStatus.percentage,
				message: 'Cancellation requested…',
			});
		} catch (error) {
			console.error('Failed to cancel processing:', error);
			dom.showError('Failed to cancel processing. Please try again.');
		} finally {
			dom.setCancelAllButtonPending(false);
		}
	}

	public async requestCancelAll(): Promise<void> {
		return this.handleCancelAll();
	}

	private async cancelJob(jobId: string): Promise<void> {
		try {
			await tauriClient.cancelProcessing(jobId);
		} catch (error) {
			console.error(`Failed to cancel job ${jobId}:`, error);
			dom.showError(`Failed to cancel job ${jobId}`);
		}
	}

	private resetToIdle(): void {
		this.isProcessing = false;
		this.batchCompletionMessageOverride = null;
		this.currentJobType = null;
		this.lastCoverArtPath = null;

		if (this.progressUnlisten) {
			this.progressUnlisten();
			this.progressUnlisten = undefined;
		}
		if (this.queueUnlisten) {
			this.queueUnlisten();
			this.queueUnlisten = undefined;
		}
		this.clearBatchCompletionTimeout();
		this.clearSingleCompletionTimeout();

		// Clear all job progress tracking
		this.jobProgress.clear();
		this.queueOrder = [];
		this.queueOrderSet.clear();
		this.lastProgressRenderByKey.clear();
		this.pendingRender = false;
		this.latestProgressEvent = null;
		renderJobList(this.jobProgress, this.queueOrder, (id) => this.cancelJob(id));

		this.updateStatus(createInitialStatus());
		this.updateConcurrencyIndicator();

		// Re-enable controls
		setJobControlsEnabled(true);
		setFileOrderLocked(false);

		// Reset art thumbnail to placeholder
		dom.resetArtThumbnail();
	}

	/**
	 * Schedule a UI render, either immediate (for terminal events) or batched via rAF.
	 * Coalesces multiple render requests within a single frame into one flush.
	 */
	private scheduleRender(immediate: boolean): void {
		if (immediate) {
			this.flushRender();
		} else if (!this.pendingRender) {
			this.pendingRender = true;
			requestAnimationFrame(() => this.flushRender());
		}
	}

	/**
	 * Execute all pending UI rendering work:
	 * - Render job list
	 * - Calculate aggregate progress
	 * - Update status panel
	 * - Update concurrency indicator
	 * - Update art thumbnail (if applicable)
	 */
	private flushRender(): void {
		this.pendingRender = false;

		// Render job list
		renderJobList(this.jobProgress, this.queueOrder, (id) => this.cancelJob(id));

		// Calculate aggregate progress and stage
		const { aggregate, stage } = this.calculateAggregateProgressAndStage();
		this.updateConcurrencyIndicator(aggregate);

		// Build status from latest event data (if available)
		const status: ProcessingStatus = {
			stage,
			percentage: aggregate.overallPercentage,
			message: formatAggregateMessage(this.jobProgress, aggregate),
			currentFile: this.latestProgressEvent?.current_file,
			etaSeconds: this.latestProgressEvent?.eta_seconds,
		};
		this.updateStatus(status);

		// Handle art thumbnail updates for batch jobs
		if (this.currentJobType === 'batch' && this.latestProgressEvent) {
			const event = this.latestProgressEvent;
			const indexedPath =
				typeof event.input_index === 'number' ? this.findFilePathByIndex(event.input_index) : null;
			if (indexedPath) {
				void this.updateArtThumbnailForFile(indexedPath);
			} else if (event.current_file) {
				const filename = extractFilenameFromProgress(event.current_file);
				if (filename) {
					const filePath = this.findFilePathByName(filename);
					if (filePath) {
						void this.updateArtThumbnailForFile(filePath);
					}
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

		// Get the first valid file for cover art
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

	private findFilePathByName(filename: string): string | null {
		return findFilePathByNameService(getCurrentFileList(), filename);
	}

	private findFilePathByIndex(index: number): string | null {
		return findFilePathByIndexService(getCurrentFileList(), index);
	}

	// Public method to check if processing is active
	public get isCurrentlyProcessing(): boolean {
		return this.isProcessing;
	}

	// Public method to get current status
	public getCurrentStatus(): ProcessingStatus {
		return { ...this.currentStatus };
	}
}

// Export a singleton instance
let statusPanelInstance: StatusPanel | null = null;

export function initStatusPanel(): StatusPanel {
	if (!statusPanelInstance) {
		statusPanelInstance = new StatusPanel();
	}
	return statusPanelInstance;
}

export function getStatusPanel(): StatusPanel | null {
	return statusPanelInstance;
}

export function triggerProcessFromStatusPanel(options?: { previewSeconds?: number }): void {
	const panel = getStatusPanel();
	if (!panel) return;
	void panel.startProcessing(options);
}

export function triggerCancelAllFromStatusPanel(): void {
	const panel = getStatusPanel();
	if (!panel) return;
	void panel.requestCancelAll();
}

export function pushStatusPanelTransientStatus(
	message: string,
	options?: { ttlMs?: number },
): void {
	dom.pushTransientStatusMessage(message, options?.ttlMs);
}

export function clearStatusPanelTransientStatusLock(): void {
	dom.clearTransientStatusMessageLock();
}
