<script lang="ts">
	import { onMount } from 'svelte';
	import { formatEtaRemaining } from '../../lib/format/eta';
	import type { ChildJobSnapshot, OperationSnapshot } from '../../types/workRuntime';
	import {
		cancelWorkOperation,
		initializeWorkCenter,
		openChildSource,
		workCenterState,
	} from './state.svelte';

	onMount(() => {
		void initializeWorkCenter();
	});

	function operationStatusLabel(status: OperationSnapshot['status']): string {
		if (status === 'accepted') return 'Accepted';
		if (status === 'running') return 'Running';
		if (status === 'cancelling') return 'Cancelling';
		if (status === 'completed') return 'Completed';
		if (status === 'cancelled') return 'Cancelled';
		if (status === 'failed') return 'Failed';
		return 'Mixed';
	}

	function acceptedQueuePosition(operation: OperationSnapshot): number | null {
		if (operation.status !== 'accepted') return null;
		const accepted = workCenterState.operations
			.filter((candidate) => candidate.status === 'accepted')
			.sort((left, right) => left.sequence - right.sequence);
		const index = accepted.findIndex(
			(candidate) => candidate.operationId === operation.operationId,
		);
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
</script>

<div class="panel work-center" aria-label="Work Center">
	<div class="work-center-header">
		<div>
			<h3>Work Center</h3>
			<p>{workCenterState.operations.length} operation{workCenterState.operations.length === 1 ? '' : 's'}</p>
		</div>
	</div>

	{#if workCenterState.errorMessage}
		<div class="work-center-error">{workCenterState.errorMessage}</div>
	{/if}

	{#if workCenterState.operations.length === 0}
		<div class="work-center-empty">No background work.</div>
	{:else}
		<div class="work-operation-list">
			{#each workCenterState.operations as operation (operation.operationId)}
				<section class={`work-operation is-${operation.status}`}>
					<div class="work-operation-topline">
						<div class="work-operation-title-group">
							<span class="work-kind">{operationKindLabel(operation.kind)}</span>
							<span class="work-title" title={operation.title}>{operation.title}</span>
						</div>
						<div class="work-operation-actions">
							<span class={`work-status is-${operation.status}`}>
								{operationStatusLabel(operation.status)}
								{#if acceptedQueuePosition(operation)}
									#{acceptedQueuePosition(operation)}
								{/if}
							</span>
							<button
								class="work-action-button"
								disabled={
									!canCancel(operation) ||
									Boolean(workCenterState.cancelPendingByOperationId[operation.operationId])
								}
								onclick={() => void cancelWorkOperation(operation.operationId)}
							>
								Cancel
							</button>
						</div>
					</div>

					<div class="work-progress-row">
						<div class="app-progress-track work-progress-track">
							<div
								class="app-progress-fill"
								style={`width: ${Math.min(100, Math.max(0, operation.progress.percentage))}%`}
							></div>
						</div>
						<span class="work-progress-value">
							{operation.progress.percentage.toFixed(0)}%
							{#if operation.status === 'running' && operation.progress.etaSeconds != null}
								· {formatEtaRemaining(operation.progress.etaSeconds)}
							{/if}
						</span>
					</div>

					<div class="work-summary" title={summaryText(operation)}>{summaryText(operation)}</div>

					{#if operation.logTail.length > 0}
						<div class="work-log-tail" aria-label="Recent operation activity">
							{#each operation.logTail as entry}
								<div>{entry.message}</div>
							{/each}
						</div>
					{/if}

					<div class="work-child-list">
						{#each operation.children as child (child.childJobId)}
							<div class={`work-child-row is-${child.status}`}>
								<span class="work-child-label" title={child.sourcePath ?? child.label}>{child.label}</span>
								<span class="work-child-status">
									{childStatusLabel(child.status)}
									{#if child.status === 'running' && child.progress.etaSeconds != null}
										· {formatEtaRemaining(child.progress.etaSeconds)}
									{/if}
								</span>
								{#if child.sourcePath}
									<button
										class="work-child-source"
										title="Open source file"
										onclick={() => void openChildSource(child)}
									>
										Source
									</button>
								{/if}
							</div>
						{/each}
					</div>
				</section>
			{/each}
		</div>
	{/if}
</div>

<style>
	.work-center {
		flex-shrink: 0;
		gap: 0.5rem;
		padding: 0.625rem 0.75rem;
	}

	.work-center-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
	}

	.work-center-header h3 {
		margin: 0;
		font-size: 0.875rem;
		font-weight: 600;
		color: var(--text-secondary);
	}

	.work-center-header p {
		margin: 0.125rem 0 0;
		font-size: 0.75rem;
		color: var(--text-muted);
	}

	.work-center-empty,
	.work-center-error,
	.work-summary,
	.work-log-tail {
		font-size: 0.75rem;
		color: var(--text-muted);
	}

	.work-log-tail {
		max-height: 5rem;
		overflow-y: auto;
		font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
	}

	.work-center-error {
		color: var(--text-error);
	}

	.work-operation-list {
		display: flex;
		max-height: 14rem;
		flex-direction: column;
		gap: 0.5rem;
		overflow-y: auto;
		padding-right: 0.125rem;
	}

	.work-operation {
		display: flex;
		flex-direction: column;
		gap: 0.375rem;
		padding: 0.5rem;
		border: 1px solid var(--border-primary);
		border-radius: 0.375rem;
		background: var(--bg-input);
	}

	.work-operation-topline,
	.work-operation-actions,
	.work-operation-title-group,
	.work-progress-row,
	.work-child-row {
		display: flex;
		align-items: center;
		min-width: 0;
	}

	.work-operation-topline {
		justify-content: space-between;
		gap: 0.5rem;
	}

	.work-operation-title-group {
		gap: 0.375rem;
	}

	.work-kind,
	.work-status {
		flex-shrink: 0;
		border-radius: 9999px;
		padding: 0.125rem 0.375rem;
		font-size: 0.6875rem;
		font-weight: 700;
		line-height: 1.2;
	}

	.work-kind {
		background: var(--bg-hover);
		color: var(--text-secondary);
	}

	.work-title,
	.work-child-label,
	.work-summary {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.work-title {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--text-primary);
	}

	.work-operation-actions {
		flex-shrink: 0;
		gap: 0.375rem;
	}

	.work-status.is-running,
	.work-status.is-accepted,
	.work-status.is-cancelling {
		background: rgba(59, 130, 246, 0.12);
		color: var(--accent-primary);
	}

	.work-status.is-completed {
		background: rgba(16, 185, 129, 0.12);
		color: var(--text-success);
	}

	.work-status.is-failed,
	.work-status.is-mixed,
	.work-status.is-cancelled {
		background: rgba(239, 68, 68, 0.12);
		color: var(--text-error);
	}

	.work-action-button,
	.work-child-source {
		margin-top: 0;
		border: 1px solid var(--border-secondary);
		border-radius: 0.25rem;
		background: var(--bg-panel);
		color: var(--text-secondary);
		font-size: 0.6875rem;
		font-weight: 600;
		line-height: 1;
	}

	.work-action-button {
		padding: 0.25rem 0.5rem;
	}

	.work-child-source {
		padding: 0.1875rem 0.375rem;
	}

	.work-action-button:disabled {
		cursor: not-allowed;
		opacity: 0.45;
	}

	.work-progress-row {
		gap: 0.5rem;
	}

	.work-progress-track {
		flex: 1 1 auto;
	}

	.work-progress-value {
		min-width: 2.5rem;
		flex-shrink: 0;
		text-align: right;
		font-size: 0.6875rem;
		color: var(--text-muted);
	}

	.work-child-list {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.work-child-row {
		gap: 0.375rem;
		border-top: 1px solid var(--border-primary);
		padding-top: 0.25rem;
		font-size: 0.75rem;
	}

	.work-child-label {
		flex: 1 1 auto;
		color: var(--text-secondary);
	}

	.work-child-status {
		min-width: 4.5rem;
		flex-shrink: 0;
		text-align: right;
		color: var(--text-muted);
	}
</style>
