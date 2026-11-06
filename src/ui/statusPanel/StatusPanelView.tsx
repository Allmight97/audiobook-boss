import { For, Show, createMemo, createSignal } from 'solid-js';
import type { JSX } from '@solidjs/web';

import type { JobListItem } from '../../app/processing';
import { useAppRuntime } from '../../app/runtime';
import { Button, CoverImage, Progress } from '../foundation';
import './statusPanelView.css';

function jobStatusLabel(item: JobListItem): string {
	return `${item.statusText} (${item.percentage.toFixed(1)}%)`;
}

const JOB_STATUS_CLASS: Record<JobListItem['status'], string> = {
	queued: 'is-queued',
	running: 'is-active',
	completed: 'is-complete',
	skipped: 'is-complete',
	failed: 'is-failed',
	cancelled: 'is-cancelled',
};

function countItems(
	items: ReadonlyArray<JobListItem>,
	statuses: ReadonlyArray<JobListItem['status']>,
): number {
	return items.filter((item) => statuses.includes(item.status)).length;
}

function activeStageLabel(stage: JobListItem['stage']): string {
	switch (stage) {
		case 'pending':
		case 'analyzing':
			return 'analyzing';
		case 'converting':
			return 'converting';
		case 'writing':
		case 'committing':
		case 'cleaning':
			return 'writing';
		case 'complete':
		case 'failed':
		case 'cancelled':
			return 'running';
	}
}

function activeChipLabel(items: ReadonlyArray<JobListItem>): string | null {
	const activeItems = items.filter((item) => item.status === 'running');
	if (activeItems.length === 0) return null;
	const labels = new Set(activeItems.map((item) => activeStageLabel(item.stage)));
	const label = labels.size === 1 ? Array.from(labels)[0] : 'running';
	return `${activeItems.length} ${label}`;
}

export function StatusPanelView(): JSX.Element {
	const processing = useAppRuntime().processing;
	const view = processing.status;
	const startProcessing = processing.start;
	const [queueExpanded, setQueueExpanded] = createSignal(false);
	const items = createMemo(() => view().jobItems);
	const activeChip = createMemo(() => activeChipLabel(items()));
	const queuedCount = createMemo(() => countItems(items(), ['queued']));
	const completeCount = createMemo(() => countItems(items(), ['completed', 'skipped']));
	const failedCount = createMemo(() => countItems(items(), ['failed']));
	const cancelledCount = createMemo(() => countItems(items(), ['cancelled']));

	return (
		<div class="panel status-panel">
			<div class="status-panel-content">
				<div class="art-thumbnail">
					<CoverImage
						src={view().coverArtSrc}
						alt="Cover Art"
						class="status-cover-image"
						eager
						missing={<span>Art</span>}
					/>
				</div>
				<div class="progress-details">
					<div class="status-progress-header">
						<span class="status-progress-label">
							Progress: <span class="property-value">{view().progressPercentage.toFixed(1)}%</span>
						</span>
						<span class="status-text">{view().statusText}</span>
					</div>
					<Progress
						value={view().progressPercentage}
						label="Progress"
						active={view().progressActive}
					/>
					<div class="muted-text status-step" style={{ color: view().stepColor }}>
						{view().stepText}
					</div>
					<div class="muted-text status-concurrency">{view().concurrencyText}</div>
					<Show when={items().length > 0}>
						<div class="queue-summary-row">
							<section class="queue-chip-group" aria-label="Queue status summary">
								<Show when={activeChip()}>
									{(label) => <span class="queue-chip is-active">{label()}</span>}
								</Show>
								<Show when={queuedCount() > 0}>
									<span class="queue-chip is-queued">{queuedCount()} queued</span>
								</Show>
								<Show when={completeCount() > 0}>
									<span class="queue-chip is-complete">{completeCount()} complete</span>
								</Show>
								<Show when={failedCount() > 0}>
									<span class="queue-chip is-failed">{failedCount()} failed</span>
								</Show>
								<Show when={cancelledCount() > 0}>
									<span class="queue-chip is-cancelled">{cancelledCount()} cancelled</span>
								</Show>
							</section>
							<button
								class="queue-toggle-button"
								type="button"
								aria-expanded={queueExpanded() ? 'true' : 'false'}
								aria-controls="job-list"
								onClick={() => setQueueExpanded(!queueExpanded())}
							>
								{queueExpanded() ? 'Hide queue' : 'View queue'}
							</button>
						</div>
					</Show>
					<div
						id="job-list"
						class="queue-job-list"
						hidden={!queueExpanded() || items().length === 0}
					>
						<For each={items()} keyed={(item) => item.childJobId}>
							{(item) => (
								<div class={`queue-job-row ${JOB_STATUS_CLASS[item().status]}`}>
									<span class="queue-job-label">{item().label}</span>
									<span class="queue-job-status">{jobStatusLabel(item())}</span>
									<button
										class="job-cancel-button"
										type="button"
										disabled={!item().canCancel}
										onClick={() => processing.cancelJob(item().childJobId)}
									>
										Cancel
									</button>
								</div>
							)}
						</For>
					</div>
				</div>
				<div class="status-actions">
					<Button
						id="process-button"
						tone="primary"
						disabled={processing.isProcessing()}
						onClick={() => void startProcessing()}
					>
						Start Processing
					</Button>
					<Button
						disabled={!processing.previewCancellable()}
						onClick={() => processing.cancelAll()}
					>
						Cancel
					</Button>
				</div>
			</div>
		</div>
	);
}
