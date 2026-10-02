import { For, Show } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { formatEtaRemaining } from '../../lib/format/eta';
import { useAppRuntime } from '../../app/runtime';
import { Progress } from '../foundation';
import type { ChildJobSnapshot, OperationSnapshot } from '../../types/workRuntime';
import './workCenterView.css';

function completedLabel(
	label: string,
	timing: { startedAtMs?: number; finishedAtMs?: number },
): string {
	if (timing.startedAtMs == null || timing.finishedAtMs == null) return label;
	const totalSeconds = Math.max(0, Math.round((timing.finishedAtMs - timing.startedAtMs) / 1000));
	const minutes = Math.floor(totalSeconds / 60);
	const seconds = String(totalSeconds % 60).padStart(2, '0');
	return `${label} in ${String(minutes).padStart(2, '0')}:${seconds}`;
}

function operationStatusLabel(operation: OperationSnapshot): string {
	const { status } = operation;
	if (status === 'accepted') return 'Accepted';
	if (status === 'running') return 'Running';
	if (status === 'cancelling') return 'Cancelling';
	if (status === 'completed') return completedLabel('Completed', operation);
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
	if (kind === 'remoteAcquisition') return 'Acquisition';
	return 'Metadata';
}

function childStatusLabel(child: ChildJobSnapshot): string {
	const { status } = child;
	if ((status === 'queued' || status === 'running') && child.cancelRequested) return 'Cancelling';
	if (status === 'queued') return 'Queued';
	if (status === 'running') return 'Running';
	if (status === 'completed') return completedLabel('Done', child);
	if (status === 'skipped') return 'Skipped';
	if (status === 'cancelled') return 'Cancelled';
	return 'Failed';
}

/** Names the host file manager; WebView user agents identify the OS. */
function revealLabel(): string {
	const agent = typeof navigator === 'undefined' ? '' : navigator.userAgent;
	if (agent.includes('Macintosh')) return 'Show in Finder';
	if (agent.includes('Windows')) return 'Show in File Explorer';
	return 'Show in Folder';
}

function canCancel(operation: OperationSnapshot): boolean {
	return operation.cancellable && !operation.cancelRequested;
}

/** A single-title operation already has the operation Cancel button. */
function canCancelTitle(operation: OperationSnapshot, child: ChildJobSnapshot): boolean {
	return (
		operation.children.length > 1 &&
		canCancel(operation) &&
		child.cancellable &&
		!child.cancelRequested
	);
}

function summaryText(operation: OperationSnapshot): string {
	if (operation.terminalSummary) return operation.terminalSummary.message;
	return operation.progress.message;
}

/** How a Save's edit to this title's output went, when there was one. */
function outputUpdateText(child: ChildJobSnapshot): string | null {
	const status = child.outputUpdate?.status;
	switch (status?.kind) {
		case 'waiting':
			return 'Tag edit waiting to be written.';
		case 'applied':
			return 'Tags updated.';
		case 'failed':
			return `Tags not updated: ${status.message}`;
		default:
			return null;
	}
}

export function WorkCenterView(): JSX.Element {
	const runtime = useAppRuntime();
	const workOperations = runtime.workOperations;
	const offerFor = (operation: OperationSnapshot, child: ChildJobSnapshot) =>
		runtime.processing
			.restartOffers()
			.find(
				(offer) => offer.operationId === operation.operationId && offer.titleId === child.inputId,
			);
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
												{operationStatusLabel(operation)}
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
										<Progress value={operation.progress.percentage} class="work-progress-track" />
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
												<>
													<div class={`work-child-row is-${child.status}`}>
														<span class="work-child-label" title={child.sourcePath ?? child.label}>
															{child.label}
														</span>
														<span class="work-child-status">
															{childStatusLabel(child)}
															<Show
																when={
																	child.status === 'running' && child.progress.etaSeconds != null
																}
															>
																· {formatEtaRemaining(child.progress.etaSeconds ?? 0)}
															</Show>
														</span>
														<Show when={canCancelTitle(operation, child)}>
															<button
																class="work-child-action"
																type="button"
																title={`Cancel ${child.label} only`}
																onClick={() =>
																	void workOperations.cancel(
																		operation.operationId,
																		child.childJobId,
																	)
																}
															>
																Cancel
															</button>
														</Show>
														<Show when={offerFor(operation, child)}>
															{(offer) => (
																<>
																	<button
																		class="work-child-action"
																		type="button"
																		title={`Restart at ${offer().to}`}
																		onClick={() => void runtime.processing.restart(offer())}
																	>
																		Restart
																	</button>
																	<button
																		class="work-child-action"
																		type="button"
																		title={`Keep ${offer().from} and apply the latest tags`}
																		onClick={() => void runtime.processing.keepLocation(offer())}
																	>
																		Keep Location
																	</button>
																</>
															)}
														</Show>
														<Show when={child.status === 'completed' && child.outputPath}>
															<button
																class="work-child-action"
																type="button"
																title={child.outputPath ?? undefined}
																onClick={() => void workOperations.revealOutput(child)}
															>
																{revealLabel()}
															</button>
														</Show>
													</div>
													<Show
														when={
															child.status === 'failed'
																? child.message
																: (child.supplementalWarning ?? outputUpdateText(child))
														}
													>
														{(reason) => (
															<div class="work-child-reason" title={reason()}>
																{reason()}
															</div>
														)}
													</Show>
												</>
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
