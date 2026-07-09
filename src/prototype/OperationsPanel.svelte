<script lang="ts">
	export type OpsMode = 'collapsed' | 'open' | 'pinned';

	type Props = {
		mode: OpsMode;
		expandedOp: number;
		onSetMode: (mode: OpsMode) => void;
		onToggleExpanded: (index: number) => void;
	};

	let { mode, expandedOp, onSetMode, onToggleExpanded }: Props = $props();

	function toggleDisclosure(): void {
		if (mode === 'pinned') return;
		onSetMode(mode === 'collapsed' ? 'open' : 'collapsed');
	}

	function togglePin(): void {
		onSetMode(mode === 'pinned' ? 'open' : 'pinned');
	}
</script>

<div class="proto-ops" class:open={mode !== 'collapsed'} data-testid="ops-panel">
	<div class="proto-ops-bar">
		<button type="button" class="btn-pill btn-pill-secondary proto-btn-sm" aria-label="Preview 30 seconds">
			▶ 30s
		</button>
		<div class="proto-transport-track">
			<div class="app-progress-track">
				<div class="app-progress-fill" style="width: 64%"></div>
			</div>
		</div>
		<span class="proto-transport-label">The Way of Kings · 64% · 04:02 left</span>
		<span class="proto-ops-meta">
			<span class="proto-badge proto-badge-info">1 running</span>
			<span class="proto-badge proto-badge-mut">1 queued</span>
			<span class="proto-badge proto-badge-ok">2 done</span>
			<span>5 books · <span class="proto-mono">62:46:27 · 1.72 GB</span></span>
			<button
				type="button"
				class="proto-pin"
				class:on={mode === 'pinned'}
				aria-pressed={mode === 'pinned'}
				aria-label={mode === 'pinned' ? 'Unpin operations panel' : 'Pin operations panel open'}
				onclick={togglePin}
				data-testid="ops-pin"
			>
				⚲ {mode === 'pinned' ? 'pinned' : 'unpinned'}
			</button>
			<button
				type="button"
				class="proto-disclosure"
				aria-expanded={mode !== 'collapsed'}
				aria-controls="proto-ops-body"
				aria-label={mode === 'collapsed' ? 'Expand operations panel' : 'Collapse operations panel'}
				onclick={toggleDisclosure}
				data-testid="ops-disclosure"
			>
				▾
			</button>
		</span>
	</div>
	<div id="proto-ops-body" class="proto-ops-body">
		<div class="proto-op" class:open={expandedOp === 0}>
			<div class="proto-op-row">
				<button
					type="button"
					class="proto-op-disclosure"
					aria-expanded={expandedOp === 0}
					onclick={() => onToggleExpanded(0)}
				>
					<span class="proto-badge proto-badge-info">merge</span>
					<span class="proto-op-title">The Way of Kings — 3 files → M4B</span>
					<span class="proto-mono proto-op-pct">64%</span>
				</button>
				<button type="button" class="btn-pill btn-pill-secondary proto-btn-xs" aria-label="Cancel merge operation">
					Cancel
				</button>
			</div>
			<div class="proto-op-detail">
				<div class="proto-lane">
					<span>analysis</span>
					<div class="app-progress-track proto-lane-track">
						<div class="app-progress-fill" style="width:100%; background: var(--text-success)"></div>
					</div>
					<span class="proto-mono">done</span>
				</div>
				<div class="proto-lane">
					<span>encode</span>
					<div class="app-progress-track proto-lane-track">
						<div class="app-progress-fill" style="width:64%"></div>
					</div>
					<span class="proto-mono">04:02</span>
				</div>
				<div class="proto-lane">
					<span>commit</span>
					<div class="app-progress-track proto-lane-track">
						<div class="app-progress-fill" style="width:0%"></div>
					</div>
					<span class="proto-mono">—</span>
				</div>
				<div class="proto-op-log">
					<b>10:22:11</b> encoding chunk 8/12 · 12.1 MB/s<br />
					<b>10:22:40</b> chapters synthesized · 75 markers
				</div>
			</div>
		</div>
		<div class="proto-op" class:open={expandedOp === 1}>
			<button type="button" class="proto-op-row proto-op-disclosure-only" onclick={() => onToggleExpanded(1)}>
				<span class="proto-badge proto-badge-mut">queued</span>
				<span class="proto-op-title muted-text">Emergent Strategy — batch encode</span>
				<span class="proto-mono proto-op-pct muted-text">#2</span>
			</button>
		</div>
		<div class="proto-op proto-op-done" class:open={expandedOp === 2}>
			<button type="button" class="proto-op-row proto-op-disclosure-only" onclick={() => onToggleExpanded(2)}>
				<span class="proto-badge proto-badge-ok">done</span>
				<span class="proto-op-title">The Martian — batch encode</span>
				<span class="proto-mono proto-op-pct muted-text">2m ago</span>
			</button>
			<div class="proto-op-detail">
				<div class="proto-op-log">
					<b>terminal</b> success · output verified · 298.4 MB
				</div>
			</div>
		</div>
	</div>
</div>

<style>
	.proto-ops {
		border-top: 1px solid var(--border-primary);
		background: var(--bg-panel);
		flex-shrink: 0;
	}

	.proto-ops-bar {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		width: 100%;
		padding: 9px var(--space-4);
		color: inherit;
		text-align: left;
	}

	.proto-transport-track {
		flex: 1;
		max-width: 23.75rem;
	}

	.proto-transport-label {
		font-size: var(--text-sm);
		color: var(--text-secondary);
		white-space: nowrap;
	}

	.proto-ops-meta {
		margin-left: auto;
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-sm);
		color: var(--text-muted);
		flex-wrap: wrap;
		justify-content: flex-end;
	}

	.proto-mono {
		font-family: var(--font-mono);
	}

	.proto-pin,
	.proto-disclosure {
		margin-top: 0;
		background: none;
		border: none;
		color: var(--text-placeholder);
		font-size: var(--text-sm);
		padding: 4px;
		cursor: pointer;
	}

	.proto-pin.on {
		color: var(--accent-primary-hover);
	}

	.proto-ops-body {
		display: none;
		max-height: 13.75rem;
		overflow: auto;
		padding: 0 var(--space-4) var(--space-3);
		flex-direction: column;
		gap: var(--space-2);
	}

	.proto-ops.open .proto-ops-body {
		display: flex;
	}

	.proto-op {
		border: 1px solid var(--border-primary);
		border-radius: var(--radius-md);
	}

	.proto-op-done {
		opacity: 0.6;
	}

	.proto-op-row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		width: 100%;
		padding: var(--space-2) var(--space-3);
		border: none;
		background: transparent;
		font-size: var(--text-sm);
		color: inherit;
		text-align: left;
	}

	.proto-op-disclosure,
	.proto-op-disclosure-only {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		flex: 1;
		min-width: 0;
		margin-top: 0;
		padding: 0;
		border: none;
		background: transparent;
		font-size: var(--text-sm);
		color: inherit;
		cursor: pointer;
		text-align: left;
	}

	.proto-op-disclosure:hover,
	.proto-op-disclosure-only:hover {
		background: var(--bg-input);
	}

	.proto-op-title {
		flex: 1;
		font-weight: 500;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.proto-op-pct {
		color: var(--text-muted);
	}

	.proto-op-detail {
		display: none;
		padding: 2px var(--space-3) var(--space-2);
	}

	.proto-op.open .proto-op-detail {
		display: block;
	}

	.proto-lane {
		display: flex;
		gap: var(--space-2);
		align-items: center;
		font-size: var(--text-xs);
		color: var(--text-muted);
		padding: 4px 0;
	}

	.proto-lane span:first-child {
		width: 4.375rem;
		flex-shrink: 0;
	}

	.proto-lane-track {
		flex: 1;
	}

	.proto-op-log {
		margin-top: var(--space-2);
		background: color-mix(in srgb, var(--bg-main) 80%, #000);
		border-radius: var(--radius-sm);
		padding: var(--space-2) var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		line-height: 1.7;
		color: var(--text-muted);
	}

	.proto-op-log b {
		color: var(--text-secondary);
		font-weight: 500;
	}

	.proto-badge {
		font-size: var(--text-xs);
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		padding: 2px 8px;
		border-radius: var(--radius-pill);
		white-space: nowrap;
	}

	.proto-badge-ok {
		background: color-mix(in srgb, var(--text-success) 14%, transparent);
		color: var(--text-success);
	}

	.proto-badge-info {
		background: color-mix(in srgb, var(--accent-primary) 16%, transparent);
		color: var(--accent-primary-hover);
	}

	.proto-badge-mut {
		background: var(--bg-input);
		color: var(--text-muted);
	}

	.proto-btn-sm {
		padding: 5px 12px !important;
		font-size: var(--text-sm) !important;
	}

	.proto-btn-xs {
		padding: 3px 10px !important;
		font-size: var(--text-xs) !important;
	}
</style>
