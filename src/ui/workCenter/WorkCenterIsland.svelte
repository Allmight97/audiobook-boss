<script lang="ts">
	import { onMount } from 'svelte';
	import type { ChildJobSnapshot, OperationSnapshot, ResourceLane } from '../../types/workRuntime';
	import {
		cancelWorkOperation,
		initializeWorkCenter,
		openChildSource,
		startWorkCenterClock,
		workCenterClock,
		workCenterState,
	} from './state.svelte';
	import { isTerminalOperationStatus } from './model';
	import { formatRelativeTime } from './relativeTime';

	let expandedOperationIds = $state<Set<string>>(new Set());

	onMount(() => {
		void initializeWorkCenter();
		return startWorkCenterClock();
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
		if (status === 'accepted') return 'queued';
		if (status === 'running') return 'running';
		if (status === 'cancelling') return 'cancelling';
		if (status === 'completed') return 'done';
		if (status === 'cancelled') return 'cancelled';
		if (status === 'failed') return 'failed';
		return 'mixed';
	}

	function operationStatusVariant(status: OperationSnapshot['status']): 'info' | 'ok' | 'warn' | 'muted' {
		if (status === 'accepted') return 'muted';
		if (status === 'running' || status === 'cancelling') return 'info';
		if (status === 'completed') return 'ok';
		return 'warn';
	}

	function operationPositionText(operation: OperationSnapshot): string {
		if (operation.status === 'accepted') {
			// Dispatch is FIFO by sequence; the list renders newest-first, so
			// position must count by ascending sequence, not render order.
			const position = workCenterState.operations
				.filter((candidate) => candidate.status === 'accepted')
				.sort((a, b) => a.sequence - b.sequence)
				.findIndex((candidate) => candidate.operationId === operation.operationId);
			return `#${position + 1}`;
		}
		if (isTerminalOperationStatus(operation.status)) {
			return operation.finishedAtMs == null
				? 'just now'
				: formatRelativeTime(operation.finishedAtMs, workCenterClock.nowMs);
		}
		return `${operation.progress.percentage.toFixed(0)}%`;
	}

	function formatLogTimestamp(timestampMs: number): string {
		const timestamp = new Date(timestampMs);
		return [timestamp.getHours(), timestamp.getMinutes(), timestamp.getSeconds()]
			.map((value) => String(value).padStart(2, '0'))
			.join(':');
	}

	function isNestedInteractiveTarget(event: KeyboardEvent): boolean {
		const target = event.target;
		if (!(target instanceof Element)) return false;
		const interactive = target.closest('button, a, input, select, textarea, [role="button"]');
		return interactive !== null && interactive !== event.currentTarget;
	}

	function handleOperationRowKeydown(event: KeyboardEvent, operationId: string): void {
		if (event.key !== 'Enter' && event.key !== ' ') return;
		if (isNestedInteractiveTarget(event)) return;
		event.preventDefault();
		toggleExpanded(operationId);
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
				<article
					class:expanded={expandedOperationIds.has(operation.operationId)}
					class:terminal={isTerminalOperationStatus(operation.status)}
					class="op-card"
				>
					<div
						class="op-row"
						role="button"
						tabindex="0"
							aria-expanded={expandedOperationIds.has(operation.operationId)}
							aria-label={`${expandedOperationIds.has(operation.operationId) ? 'Collapse' : 'Expand'} ${operation.title}`}
							onclick={() => toggleExpanded(operation.operationId)}
							onkeydown={(event) => handleOperationRowKeydown(event, operation.operationId)}
						>
							<span class={`app-badge app-badge-${operationStatusVariant(operation.status)}`}>
								{operationStatusLabel(operation.status)}
							</span>
							<span class="work-operation-title" title={operation.title}>{operation.title}</span>
							<span class="work-operation-position">{operationPositionText(operation)}</span>
						{#if canCancel(operation)}
							<button
								type="button"
								class="pill pill-ghost pill-xs"
								aria-label={`Cancel ${operation.title}`}
								disabled={Boolean(workCenterState.cancelPendingByOperationId[operation.operationId])}
								onclick={(event) => {
									event.stopPropagation();
									void cancelWorkOperation(operation.operationId);
								}}
							>
								Cancel
							</button>
						{/if}
					</div>

					{#if expandedOperationIds.has(operation.operationId)}
						<div class="op-detail">
							{#each operation.lanes as lane (lane)}
								<div class="lane" data-testid={`operation-lane-${lane}`}>
									<span>{laneLabel(lane)}</span>
									<div
										class="app-progress-track work-operation-lane-track"
										role="progressbar"
										aria-label={`${laneLabel(lane)} progress`}
										aria-valuemin="0"
										aria-valuemax="100"
										aria-valuenow={Math.round(lanePercentage(operation, lane))}
									>
										<div
										class="app-progress-fill"
										class:lane-fill-done={lanePercentage(operation, lane) >= 100}
										style={`width: ${lanePercentage(operation, lane)}%`}
									></div>
									</div>
									<span>{laneDetail(operation, lane)}</span>
								</div>
							{/each}

							<div class="op-log">
								{#each operation.logTail as entry}
									<b>{formatLogTimestamp(entry.timestampMs)}</b> {entry.message}<br />
								{/each}
							</div>

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
												class="pill pill-ghost pill-xs"
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
	.op-detail,
	.work-child-list {
		display: flex;
		flex-direction: column;
	}

	/* Mock: a finished lane's fill reads green (working_prototype_mock.html:180). */
	.lane-fill-done {
		background: var(--text-success);
	}

	.work-center,
	.work-operation-list,
	.op-detail {
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

	.op-card.terminal {
		opacity: 0.6;
	}

	.op-row,
	.lane,
	.work-child-row {
		display: flex;
		align-items: center;
		min-width: 0;
	}

	.op-row {
		min-width: 0;
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
		font-weight: 500;
	}

	.work-operation-position {
		color: var(--text-muted);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
	}

	.lane > :first-child {
		width: 4.375rem;
	}

	.lane > :last-child {
		width: 4.75rem;
		text-align: right;
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
