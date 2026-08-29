import { For, Show, createMemo, createSignal, type JSX } from 'solid-js';
import {
	startProcessingAtom,
	statusViewAtom,
	triggerCancelAllFromStatusPanel,
	type JobListItem,
} from '../../app/processing';
import { useAtomSet, useAtomValue } from '../../app/runtime/solid';
import type { EventStage } from '../../types/events';
import './statusPanelView.css';

function jobPercentageText(item: JobListItem): string {
	return typeof item.percentage === 'number' ? ` (${item.percentage.toFixed(1)}%)` : '';
}

function jobStatusLabel(item: JobListItem): string {
	return `${item.statusText}${jobPercentageText(item)}`;
}

function jobStatusClass(item: JobListItem): string {
	if (item.status === 'processing') return 'is-active';
	if (item.status === 'queued') return 'is-queued';
	if (item.status === 'completed' || item.status === 'skipped') return 'is-complete';
	if (item.status === 'failed') return 'is-failed';
	if (item.status === 'cancelled') return 'is-cancelled';
	return '';
}

function countItems(
	items: ReadonlyArray<JobListItem>,
	statuses: ReadonlyArray<JobListItem['status']>,
): number {
	return items.filter((item) => statuses.includes(item.status)).length;
}

function activeStageLabel(stage?: EventStage): string {
	if (stage === 'writing') return 'writing';
	if (stage === 'converting') return 'converting';
	if (stage === 'analyzing') return 'analyzing';
	return 'running';
}

function activeChipLabel(items: ReadonlyArray<JobListItem>): string | null {
	const activeItems = items.filter((item) => item.status === 'processing');
	if (activeItems.length === 0) return null;
	const labels = new Set(activeItems.map((item) => activeStageLabel(item.stage)));
	const label = labels.size === 1 ? Array.from(labels)[0] : 'running';
	return `${activeItems.length} ${label}`;
}

export function StatusPanelView(): JSX.Element {
	const view = useAtomValue(() => statusViewAtom);
	const startProcessing = useAtomSet(() => startProcessingAtom);
	const [queueExpanded, setQueueExpanded] = createSignal(false);
	const items = createMemo(() => view().jobItems);
	const activeChip = createMemo(() => activeChipLabel(items()));
	const queuedCount = createMemo(() => countItems(items(), ['queued']));
	const completeCount = createMemo(() => countItems(items(), ['completed', 'skipped']));
	const failedCount = createMemo(() => countItems(items(), ['failed']));
	const cancelledCount = createMemo(() => countItems(items(), ['cancelled']));
	const canCancelForeground = createMemo(() =>
		items().some((item) => item.canCancel && item.cancelId),
	);

	return (
		<div class="panel status-panel">
			<div class="status-panel-content">
				<div class="art-thumbnail">
					<Show when={view().coverArtDataUrl} fallback={<span>Art</span>}>
						{(dataUrl) => (
							<img
								src={dataUrl()}
								alt="Cover Art"
								style={{
									width: '100%',
									height: '100%',
									'object-fit': 'cover',
									'border-radius': '0.25rem',
								}}
							/>
						)}
					</Show>
				</div>
				<div class="progress-details">
					<div class="flex justify-between mb-0.5">
						<span class="text-xs">
							Progress:{' '}
							<span class="property-value" id="percentage-processed">
								{view().progressPercentage.toFixed(1)}%
							</span>
						</span>
						<span id="status-text" class="text-xs font-semibold">
							{view().statusText}
						</span>
					</div>
					<div class="app-progress-track">
						<div
							id="progress-bar"
							class="app-progress-fill progress-bar-fg"
							style={{ width: `${view().progressPercentage}%` }}
						/>
					</div>
					<div id="step-text" class="text-xs muted-text mt-0.5" style={{ color: view().stepColor }}>
						{view().stepText}
					</div>
					<div id="concurrency-status" class="text-xs muted-text mt-1">
						{view().concurrencyText}
					</div>
					<Show when={items().length > 0}>
						<div class="queue-summary-row" id="queue-summary">
							<section class="queue-chip-group" aria-label="Queue status summary">
								<Show when={activeChip()}>
									{(label) => (
										<span class="queue-chip is-active" data-testid="queue-chip-active">
											{label()}
										</span>
									)}
								</Show>
								<Show when={queuedCount() > 0}>
									<span class="queue-chip is-queued" data-testid="queue-chip-queued">
										{queuedCount()} queued
									</span>
								</Show>
								<Show when={completeCount() > 0}>
									<span class="queue-chip is-complete" data-testid="queue-chip-complete">
										{completeCount()} complete
									</span>
								</Show>
								<Show when={failedCount() > 0}>
									<span class="queue-chip is-failed" data-testid="queue-chip-failed">
										{failedCount()} failed
									</span>
								</Show>
								<Show when={cancelledCount() > 0}>
									<span class="queue-chip is-cancelled" data-testid="queue-chip-cancelled">
										{cancelledCount()} cancelled
									</span>
								</Show>
							</section>
							<button
								id="queue-toggle-button"
								class="queue-toggle-button"
								type="button"
								aria-expanded={queueExpanded()}
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
						<For each={items()}>
							{(item) => (
								<div class={`queue-job-row ${jobStatusClass(item)}`}>
									<span class="queue-job-label">{item.label}</span>
									<span class="queue-job-status">{jobStatusLabel(item)}</span>
									<button
										id={`cancel-${item.key}`}
										class="job-cancel-button"
										type="button"
										disabled={view().cancelAllPending || !item.canCancel || !item.cancelId}
										onClick={() => triggerCancelAllFromStatusPanel()}
									>
										Cancel
									</button>
								</div>
							)}
						</For>
					</div>
				</div>
				<div class="status-actions">
					<button
						id="process-button"
						class="btn-pill btn-pill-primary"
						type="button"
						onClick={() => void startProcessing(undefined)}
					>
						Start Processing
					</button>
					<button
						id="cancel-all-button"
						class="btn-pill btn-pill-secondary"
						type="button"
						disabled={view().cancelAllPending || !view().isProcessing || !canCancelForeground()}
						onClick={() => triggerCancelAllFromStatusPanel()}
					>
						Cancel
					</button>
				</div>
			</div>
		</div>
	);
}
