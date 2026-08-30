import { For, Show, type JSX } from 'solid-js';
import { formatEtaRemaining } from '../../lib/format/eta';
import { useAppRuntime } from '../../app/runtime';
import type { ChildJobSnapshot, OperationSnapshot } from '../../types/workRuntime';
import './workCenterView.css';

function operationStatusLabel(status: OperationSnapshot['status']): string {
	if (status === 'accepted') return 'Accepted';
	if (status === 'running') return 'Running';
	if (status === 'cancelling') return 'Cancelling';
	if (status === 'completed') return 'Completed';
	if (status === 'cancelled') return 'Cancelled';
	if (status === 'failed') return 'Failed';
	return 'Mixed';
}

function acceptedQueuePosition(
	operation: OperationSnapshot,
	operations: ReadonlyArray<OperationSnapshot>,
): number | null {
	if (operation.status !== 'accepted') return null;
	const accepted = operations
		.filter((candidate) => candidate.status === 'accepted')
		.sort((left, right) => left.sequence - right.sequence);
	const index = accepted.findIndex((candidate) => candidate.operationId === operation.operationId);
	return index >= 0 ? index + 1 : null;
}

function operationKindLabel(kind: OperationSnapshot['kind']): string {
	if (kind === 'processingBatch') return 'Batch';
	if (kind === 'processingMerge') return 'Merge';
	if (kind === 'remoteAcquisition') return 'Acquisition';
	return 'Metadata';
}

function childStatusLabel(status: ChildJobSnapshot['status']): string {
	if (status === 'queued') return 'Queued';
	if (status === 'running') return 'Running';
	if (status === 'completed') return 'Done';
	if (status === 'skipped') return 'Skipped';
	if (status === 'cancelled') return 'Cancelled';
	return 'Failed';
}

function canCancel(operation: OperationSnapshot): boolean {
	return operation.cancellable && !operation.cancelRequested;
}

function summaryText(operation: OperationSnapshot): string {
	if (operation.terminalSummary) return operation.terminalSummary.message;
	return operation.progress.message;
}

export function WorkCenterView(): JSX.Element {
	const workOperations = useAppRuntime().workOperations;
	const view = workOperations.view;

	return (
		<section class="panel work-center" aria-label="Work Center">
			<div class="work-center-header">
				<div>
					<h3>Work Center</h3>
					<p>
						{view().operations.length} operation{view().operations.length === 1 ? '' : 's'}
					</p>
				</div>
			</div>

			<Show when={view().errorMessage}>
				{(message) => <div class="work-center-error">{message()}</div>}
			</Show>

			<Show
				when={view().operations.length > 0}
				fallback={<div class="work-center-empty">No background work.</div>}
			>
				<div class="work-operation-list">
					<For each={view().operations}>
						{(operation) => {
							const queuePosition = () => acceptedQueuePosition(operation, view().operations);
							return (
								<section class={`work-operation is-${operation.status}`}>
									<div class="work-operation-topline">
										<div class="work-operation-title-group">
											<span class="work-kind">{operationKindLabel(operation.kind)}</span>
											<span class="work-title" title={operation.title}>
												{operation.title}
											</span>
										</div>
										<div class="work-operation-actions">
											<span class={`work-status is-${operation.status}`}>
												{operationStatusLabel(operation.status)}
												<Show when={queuePosition()}>{(position) => <> #{position()}</>}</Show>
											</span>
											<button
												class="work-action-button"
												type="button"
												disabled={
													!canCancel(operation) ||
													Boolean(view().cancelPendingByOperationId[operation.operationId])
												}
												onClick={() => void workOperations.cancel(operation.operationId)}
											>
												Cancel
											</button>
										</div>
									</div>
									<div class="work-progress-row">
										<div class="app-progress-track work-progress-track">
											<div
												class="app-progress-fill"
												style={{
													width: `${Math.min(100, Math.max(0, operation.progress.percentage))}%`,
												}}
											/>
										</div>
										<span class="work-progress-value">
											{operation.progress.percentage.toFixed(0)}%
											<Show
												when={
													operation.status === 'running' && operation.progress.etaSeconds != null
												}
											>
												· {formatEtaRemaining(operation.progress.etaSeconds ?? 0)}
											</Show>
										</span>
									</div>
									<div class="work-summary" title={summaryText(operation)}>
										{summaryText(operation)}
									</div>
									<Show when={operation.logTail.length > 0}>
										<div class="work-log-tail" role="log" aria-label="Recent operation activity">
											<For each={operation.logTail}>{(entry) => <div>{entry.message}</div>}</For>
										</div>
									</Show>
									<div class="work-child-list">
										<For each={operation.children}>
											{(child) => (
												<div class={`work-child-row is-${child.status}`}>
													<span class="work-child-label" title={child.sourcePath ?? child.label}>
														{child.label}
													</span>
													<span class="work-child-status">
														{childStatusLabel(child.status)}
														<Show
															when={child.status === 'running' && child.progress.etaSeconds != null}
														>
															· {formatEtaRemaining(child.progress.etaSeconds ?? 0)}
														</Show>
													</span>
													<Show when={child.sourcePath}>
														<button
															class="work-child-source"
															type="button"
															title="Open source file"
															onClick={() => void workOperations.openSource(child)}
														>
															Source
														</button>
													</Show>
												</div>
											)}
										</For>
									</div>
								</section>
							);
						}}
					</For>
				</div>
			</Show>
		</section>
	);
}
