import { For, Show } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { formatEtaRemaining } from '../../lib/format/eta';
import { useAppRuntime } from '../../app/runtime';
import { fileManagerLabel } from '../../lib/platform';
import { Progress } from '../foundation';
import type { ChildJobSnapshot, OperationSnapshot } from '../../lib/generated/tauri';
import './workCenterView.css';

function completedLabel(
	label: string,
	timing: { startedAtMs: number | null; finishedAtMs: number | null },
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

function operationKindLabel(kind: OperationSnapshot['kind']): string {
	if (kind === 'processingBatch') return 'Batch';
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

function canCancelSeparateTitle(operation: OperationSnapshot, child: ChildJobSnapshot): boolean {
	return operation.children.length > 1 && child.cancellable;
}

function childReason(child: ChildJobSnapshot): string | null {
	if (child.status === 'failed') return child.message ?? null;
	const endNote = child.status === 'skipped' || child.status === 'cancelled' ? child.message : null;
	const notes = [
		endNote,
		child.supplementalWarning,
		...child.warnings,
		outputUpdateText(child),
	].filter(Boolean);
	return notes.length > 0 ? notes.join(' ') : null;
}

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
					<For each={view().operations} keyed={(operation) => operation.operationId}>
						{(operation) => (
							<section class={`work-operation is-${operation().status}`}>
								<div class="work-operation-topline">
									<div class="work-operation-title-group">
										<span class="work-kind">{operationKindLabel(operation().kind)}</span>
										<span class="work-title" title={operation().title}>
											{operation().title}
										</span>
									</div>
									<div class="work-operation-actions">
										<span class={`work-status is-${operation().status}`}>
											{operationStatusLabel(operation())}
										</span>
										<button
											class="work-action-button"
											type="button"
											disabled={
												!operation().cancellable ||
												view().cancelPendingByOperationId[operation().operationId] === true
											}
											onClick={() => void workOperations.cancel(operation().operationId)}
										>
											Cancel
										</button>
									</div>
								</div>
								<div class="work-progress-row">
									<Progress
										value={operation().progress.percentage}
										label={`${operation().title} progress`}
										active={operation().phase === 'active'}
										class="work-progress-track"
									/>
									<span class="work-progress-value">
										{operation().progress.percentage.toFixed(0)}%
										<Show
											when={
												operation().status === 'running' && operation().progress.etaSeconds != null
											}
										>
											· {formatEtaRemaining(operation().progress.etaSeconds ?? 0)}
										</Show>
									</span>
								</div>
								<div class="work-summary" title={operation().progress.message}>
									{operation().progress.message}
								</div>
								<Show when={operation().logTail.length > 0}>
									<div class="work-log-tail" role="log" aria-label="Recent operation activity">
										<For each={operation().logTail}>{(entry) => <div>{entry.message}</div>}</For>
									</div>
								</Show>
								<div class="work-child-list">
									<For each={operation().children} keyed={(child) => child.childJobId}>
										{(child) => (
											<>
												<div class={`work-child-row is-${child().status}`}>
													<span
														class="work-child-label"
														title={child().sourcePath ?? child().label}
													>
														{child().label}
													</span>
													<span class="work-child-status">
														{childStatusLabel(child())}
														<Show
															when={
																child().status === 'running' && child().progress.etaSeconds != null
															}
														>
															· {formatEtaRemaining(child().progress.etaSeconds ?? 0)}
														</Show>
													</span>
													<Show when={canCancelSeparateTitle(operation(), child())}>
														<button
															class="work-child-action"
															type="button"
															title={`Cancel ${child().label} only`}
															onClick={() =>
																void workOperations.cancel(
																	operation().operationId,
																	child().childJobId,
																)
															}
														>
															Cancel
														</button>
													</Show>
													<Show when={offerFor(operation(), child())}>
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
													<Show when={child().outputPath}>
														<button
															class="work-child-action"
															type="button"
															title={child().outputPath ?? undefined}
															onClick={() => void workOperations.revealOutput(child())}
														>
															{fileManagerLabel()}
														</button>
													</Show>
												</div>
												<Show when={childReason(child())}>
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
						)}
					</For>
				</div>
			</Show>
		</section>
	);
}
