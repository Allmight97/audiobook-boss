<script lang="ts">
	import { onMount } from 'svelte';
	import type { ChildJobSnapshot, OperationSnapshot, ResourceLane } from '../../types/workRuntime';
	import {
		cancelWorkOperation,
		initializeWorkCenter,
		openChildSource,
		workCenterState,
	} from './state.svelte';

	let expandedOperationIds = $state<Set<string>>(new Set());

	onMount(() => {
		void initializeWorkCenter();
	});

	function toggleExpanded(operationId: string): void {
		const next = new Set(expandedOperationIds);
		if (next.has(operationId)) {
			next.delete(operationId);
		} else {
			next.add(operationId);
		}
		expandedOperationIds = next;
	}

	function operationStatusLabel(status: OperationSnapshot['status']): string {
		if (status === 'accepted') return 'Queued';
		if (status === 'running') return 'Running';
		if (status === 'cancelling') return 'Cancelling';
		if (status === 'completed') return 'Done';
		if (status === 'cancelled') return 'Cancelled';
		if (status === 'failed') return 'Failed';
		return 'Mixed';
	}

	function operationStatusVariant(status: OperationSnapshot['status']): 'info' | 'ok' | 'warn' | 'muted' {
		if (status === 'accepted') return 'muted';
		if (status === 'running' || status === 'cancelling') return 'info';
		if (status === 'completed') return 'ok';
		return 'warn';
	}

	function operationPositionText(operation: OperationSnapshot): string {
		if (operation.status === 'accepted' && typeof operation.progress.currentItemIndex === 'number') {
			return `#${operation.progress.currentItemIndex + 1}`;
		}
		return `${operation.progress.percentage.toFixed(0)}%`;
	}

	function laneLabel(lane: ResourceLane): string {
		if (lane === 'encodeCpu') return 'Encode';
		if (lane === 'outputCommit') return 'Commit';
		if (lane === 'metadataWrite') return 'Metadata';
		if (lane === 'networkDownload') return 'Download';
		if (lane === 'helperMaterializer') return 'Materialize';
		return 'Analysis';
	}

	function laneChildren(operation: OperationSnapshot, lane: ResourceLane): ChildJobSnapshot[] {
		return operation.children.filter((child) => child.lane === lane);
	}

	function lanePercentage(operation: OperationSnapshot, lane: ResourceLane): number {
		const children = laneChildren(operation, lane);
		if (children.length === 0) return 0;
		return children.reduce((total, child) => total + child.progress.percentage, 0) / children.length;
	}

	function laneDetail(operation: OperationSnapshot, lane: ResourceLane): string {
		const children = laneChildren(operation, lane);
		if (children.length === 0) return '—';
		const completed = children.filter((child) => child.status === 'completed' || child.status === 'skipped').length;
		return completed === children.length ? 'done' : `${completed}/${children.length}`;
	}

	function canCancel(operation: OperationSnapshot): boolean {
		return operation.cancellable && !operation.cancelRequested;
	}

	function summaryText(operation: OperationSnapshot): string | null {
		return operation.terminalSummary?.message ?? operation.progress.message ?? null;
	}

	function childStatusLabel(status: ChildJobSnapshot['status']): string {
		if (status === 'completed') return 'Done';
		if (status === 'queued') return 'Queued';
		if (status === 'running') return 'Running';
		if (status === 'skipped') return 'Skipped';
		if (status === 'cancelled') return 'Cancelled';
		return 'Failed';
	}
</script>

<section class="work-center" aria-label="Operations list">
	{#if workCenterState.errorMessage}
		<div class="work-center-error">{workCenterState.errorMessage}</div>
	{/if}

	{#if workCenterState.operations.length === 0}
		<div class="work-center-empty">No background work.</div>
	{:else}
		<div class="work-operation-list">
			{#each workCenterState.operations as operation (operation.operationId)}
				<article class:expanded={expandedOperationIds.has(operation.operationId)} class="work-operation">
					<div class="work-operation-row">
						<button
							type="button"
							class="work-operation-disclosure"
							aria-expanded={expandedOperationIds.has(operation.operationId)}
							aria-label={`${expandedOperationIds.has(operation.operationId) ? 'Collapse' : 'Expand'} ${operation.title}`}
							onclick={() => toggleExpanded(operation.operationId)}
						>
							<span class={`app-badge app-badge-${operationStatusVariant(operation.status)}`}>
								{operationStatusLabel(operation.status)}
							</span>
							<span class="work-operation-title" title={operation.title}>{operation.title}</span>
							<span class="work-operation-position">{operationPositionText(operation)}</span>
						</button>
						{#if canCancel(operation)}
							<button
								type="button"
								class="btn-pill btn-pill-secondary btn-pill-xs"
								aria-label={`Cancel ${operation.title}`}
								disabled={Boolean(workCenterState.cancelPendingByOperationId[operation.operationId])}
								onclick={() => void cancelWorkOperation(operation.operationId)}
							>
								Cancel
							</button>
						{/if}
					</div>

					{#if expandedOperationIds.has(operation.operationId)}
						<div class="work-operation-detail">
							{#each operation.lanes as lane (lane)}
								<div class="work-operation-lane" data-testid={`operation-lane-${lane}`}>
									<span>{laneLabel(lane)}</span>
									<div class="app-progress-track work-operation-lane-track">
										<div class="app-progress-fill" style={`width: ${lanePercentage(operation, lane)}%`}></div>
									</div>
									<span>{laneDetail(operation, lane)}</span>
								</div>
							{/each}

							{#if summaryText(operation)}
								<div class="work-operation-summary">{summaryText(operation)}</div>
							{/if}

							{#if operation.warnings.length > 0 || operation.errors.length > 0}
								<div class="work-operation-messages">
									{#each operation.warnings as message (message)}
										<div>{message}</div>
									{/each}
									{#each operation.errors as message (message)}
										<div class="work-operation-error">{message}</div>
									{/each}
								</div>
							{/if}

							<div class="work-child-list">
								{#each operation.children as child (child.childJobId)}
									<div class="work-child-row">
										<span class="work-child-label" title={child.sourcePath ?? child.label}>{child.label}</span>
										<span class="work-child-status">{childStatusLabel(child.status)}</span>
										{#if child.sourcePath}
											<button
												type="button"
												class="btn-pill btn-pill-secondary btn-pill-xs"
												title="Open source file"
												onclick={() => void openChildSource(child)}
											>
												Source
											</button>
										{/if}
									</div>
								{/each}
							</div>
						</div>
					{/if}
				</article>
			{/each}
		</div>
	{/if}
</section>

<style>
	.work-center,
	.work-operation-list,
	.work-operation-detail,
	.work-child-list {
		display: flex;
		flex-direction: column;
	}

	.work-center,
	.work-operation-list,
	.work-operation-detail {
		gap: var(--space-2);
	}

	.work-center-empty,
	.work-center-error,
	.work-operation-summary,
	.work-operation-messages {
		font-size: var(--text-sm);
		color: var(--text-muted);
	}

	.work-center-error,
	.work-operation-error {
		color: var(--text-error);
	}

	.work-operation {
		border: 1px solid var(--border-primary);
		border-radius: var(--radius-md);
		background: var(--bg-input);
	}

	.work-operation-row,
	.work-operation-disclosure,
	.work-operation-lane,
	.work-child-row {
		display: flex;
		align-items: center;
		min-width: 0;
	}

	.work-operation-row {
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
	}

	.work-operation-disclosure {
		min-width: 0;
		flex: 1;
		gap: var(--space-2);
		margin-top: 0;
		border: none;
		background: transparent;
		color: inherit;
		text-align: left;
		cursor: pointer;
	}

	.work-operation-title,
	.work-child-label {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.work-operation-title {
		flex: 1;
		font-size: var(--text-sm);
		font-weight: 600;
	}

	.work-operation-position {
		color: var(--text-muted);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
	}

	.work-operation-detail {
		padding: 0 var(--space-3) var(--space-2);
	}

	.work-operation-lane {
		gap: var(--space-2);
		color: var(--text-muted);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
	}

	.work-operation-lane > :first-child,
	.work-operation-lane > :last-child {
		width: 4.75rem;
	}

	.work-operation-lane > :last-child {
		text-align: right;
	}

	.work-operation-lane-track {
		flex: 1;
	}

	.work-child-list {
		gap: var(--space-1);
	}

	.work-child-row {
		gap: var(--space-2);
		border-top: 1px solid var(--border-primary);
		padding-top: var(--space-1);
		font-size: var(--text-sm);
	}

	.work-child-label {
		flex: 1;
	}

	.work-child-status {
		color: var(--text-muted);
	}
</style>
