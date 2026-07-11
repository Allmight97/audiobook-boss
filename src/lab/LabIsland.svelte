<script lang="ts">
	import { onMount } from 'svelte';
	import { PopoverController } from '../lib/ui/popover.svelte';
	import {
		MetadataFormFieldsIsland,
		onMetadataFormActionSelectChange,
		onMetadataFormFieldInput,
	} from '../ui/metadataForm';
	import {
		applyMetadataFormLabPreset,
		metadataFormLabPresets,
		type MetadataFormLabPresetId,
	} from '../ui/metadataForm/labFixtures';

	let density = $state<'comfortable' | 'compact'>('comfortable');
	let activeMetadataPreset = $state<MetadataFormLabPresetId>('single-clean-populated');
	let popoverAnchor = $state<HTMLElement | null>(null);
	let popoverContainer = $state<HTMLElement | null>(null);
	let popoverPanel = $state<HTMLElement | null>(null);
	const popover = new PopoverController();

	$effect(() => {
		popover.setElements({
			anchor: popoverAnchor,
			container: popoverContainer,
			panel: popoverPanel,
		});
	});

	function setDensity(next: 'comfortable' | 'compact'): void {
		density = next;
		if (next === 'compact') {
			document.documentElement.dataset.density = 'compact';
		} else {
			delete document.documentElement.dataset.density;
		}
	}

	function setMetadataPreset(next: MetadataFormLabPresetId): void {
		activeMetadataPreset = next;
		applyMetadataFormLabPreset(next);
	}

	onMount(() => {
		applyMetadataFormLabPreset(activeMetadataPreset);
	});

	const colorTokens = [
		'--bg-main',
		'--bg-panel',
		'--bg-input',
		'--bg-hover',
		'--bg-drag-area',
		'--text-primary',
		'--text-secondary',
		'--text-muted',
		'--text-placeholder',
		'--text-error',
		'--text-success',
		'--text-warning',
		'--border-primary',
		'--border-secondary',
		'--border-focus',
		'--accent-primary',
		'--accent-primary-hover',
		'--progress-bg',
		'--progress-fg',
	];
	const spaceTokens = ['--space-1', '--space-2', '--space-3', '--space-4', '--space-5', '--space-6'];
	const typeTokens = ['--text-xs', '--text-sm', '--text-md', '--text-lg', '--text-xl'];
	const radiusTokens = ['--radius-sm', '--radius-md', '--radius-lg', '--radius-pill'];
	const progressStops = [0, 37, 64, 100];
	const stateFixtures = [
		{
			id: 'empty',
			label: 'Empty',
			body: 'Drop audiobook files to begin.',
			meta: 'No selection',
			tone: 'neutral',
			progress: 0,
		},
		{
			id: 'loading',
			label: 'Loading',
			body: 'Reading metadata from selected files.',
			meta: 'Pending',
			tone: 'neutral',
			progress: 12,
		},
		{
			id: 'progress',
			label: 'Progress',
			body: 'Encoding chapter 02 of 06.',
			meta: '42%',
			tone: 'active',
			progress: 42,
		},
		{
			id: 'error',
			label: 'Error',
			body: 'Chapter 03 could not be decoded.',
			meta: 'Action needed',
			tone: 'error',
			progress: 64,
		},
		{
			id: 'success',
			label: 'Terminal success',
			body: 'Output committed to the selected folder.',
			meta: 'Complete',
			tone: 'success',
			progress: 100,
		},
		{
			id: 'failure',
			label: 'Terminal failure',
			body: 'No output was written for this operation.',
			meta: 'Failed',
			tone: 'error',
			progress: 100,
		},
		{
			id: 'selected',
			label: 'Selected row',
			body: '01 - Opening Moves.m4b',
			meta: '1 selected',
			tone: 'selected',
			progress: 0,
		},
		{
			id: 'multi-selected',
			label: 'Multi-selected rows',
			body: '03 selected files share pending metadata edits.',
			meta: '3 selected',
			tone: 'selected',
			progress: 0,
		},
	] as const;

	const activeMetadataPresetDetails = $derived(
		metadataFormLabPresets.find((preset) => preset.id === activeMetadataPreset) ??
			metadataFormLabPresets[0],
	);
</script>

<div class="lab">
	<header class="lab-header panel">
		<div>
			<h2>ABB design lab</h2>
			<p class="text-xs muted-text">
				Dev-only rendering surface for tokens and primitives (#412 slice 1). Not part of the app
				build — open at /lab.html on the Vite dev server.
			</p>
		</div>
		<div class="lab-density" role="group" aria-label="Density">
			<button
				class="btn-pill"
				class:btn-pill-primary={density === 'comfortable'}
				class:btn-pill-secondary={density !== 'comfortable'}
				type="button"
				onclick={() => setDensity('comfortable')}
			>
				Comfortable
			</button>
			<button
				class="btn-pill"
				class:btn-pill-primary={density === 'compact'}
				class:btn-pill-secondary={density !== 'compact'}
				type="button"
				onclick={() => setDensity('compact')}
			>
				Compact
			</button>
		</div>
	</header>

	<section class="panel lab-section">
		<h3>Color tokens</h3>
		<div class="lab-swatches">
			{#each colorTokens as token (token)}
				<div class="lab-swatch">
					<div class="lab-swatch-chip" style={`background: var(${token})`}></div>
					<code class="text-xs">{token}</code>
				</div>
			{/each}
		</div>
	</section>

	<section class="panel lab-section">
		<h3>Scale tokens</h3>
		<div class="lab-scale-grid">
			<div>
				<h4 class="text-xs muted-text">Spacing</h4>
				{#each spaceTokens as token (token)}
					<div class="lab-space-row">
						<code class="text-xs">{token}</code>
						<div class="lab-space-bar" style={`width: var(${token})`}></div>
					</div>
				{/each}
			</div>
			<div>
				<h4 class="text-xs muted-text">Type</h4>
				{#each typeTokens as token (token)}
					<div style={`font-size: var(${token})`}>
						Aa — the quick brown fox <code class="text-xs">{token}</code>
					</div>
				{/each}
			</div>
			<div>
				<h4 class="text-xs muted-text">Radius</h4>
				<div class="lab-radius-row">
					{#each radiusTokens as token (token)}
						<div class="lab-radius-chip" style={`border-radius: var(${token})`}>
							<code class="text-xs">{token.replace('--radius-', '')}</code>
						</div>
					{/each}
				</div>
			</div>
		</div>
	</section>

	<section class="panel lab-section">
		<h3>Buttons and form controls</h3>
		<div class="lab-row">
			<button class="btn-pill btn-pill-primary" type="button">Primary</button>
			<button class="btn-pill btn-pill-secondary" type="button">Secondary</button>
			<button class="btn-pill btn-pill-secondary" type="button" disabled>Disabled</button>
			<label class="checkbox-label text-xs mb-0">
				<input type="checkbox" checked />
				<span class="option-label">Checkbox label</span>
			</label>
			<input type="text" placeholder="Text input" style="max-width: 200px" />
			<select style="max-width: 200px">
				<option>Select option</option>
			</select>
		</div>
	</section>

	<section class="panel lab-section" data-testid="badge-primitives-section">
		<h3>Badges (app-badge)</h3>
		<div class="lab-row">
			<span class="app-badge app-badge-ok">Done</span>
			<span class="app-badge app-badge-info">Running</span>
			<span class="app-badge app-badge-warn">Warning</span>
			<span class="app-badge app-badge-muted">Queued</span>
		</div>
	</section>

	<section class="panel lab-section" data-testid="pill-size-primitives-section">
		<h3>Pill sizes (btn-pill-sm / btn-pill-xs)</h3>
		<div class="lab-row">
			<button class="btn-pill btn-pill-secondary" type="button">Default</button>
			<button class="btn-pill btn-pill-secondary btn-pill-sm" type="button">Small</button>
			<button class="btn-pill btn-pill-secondary btn-pill-xs" type="button">Extra small</button>
		</div>
	</section>

	<section class="panel lab-section" data-testid="popover-primitive-section">
		<h3>Popover (app-popover)</h3>
		<div class="lab-popover-stage" bind:this={popoverContainer}>
			<button
				bind:this={popoverAnchor}
				class="btn-pill btn-pill-secondary btn-pill-sm"
				type="button"
				aria-expanded={popover.isOpen}
				onclick={() => popover.toggle()}
			>
				Popover demo
			</button>
			{#if popover.isOpen}
				<div
					bind:this={popoverPanel}
					class="app-popover lab-popover"
					role="dialog"
					aria-label="Popover primitive demo"
					tabindex="-1"
					style={`left: ${popover.position.left}px; top: ${popover.position.top}px`}
					onkeydown={(event) => popover.handleKeydown(event)}
				>
					<strong>Measured and container-clamped</strong>
					<p class="text-xs muted-text">Escape closes and returns focus to the trigger.</p>
					<button class="btn-pill btn-pill-primary btn-pill-xs" type="button" onclick={() => popover.close()}>
						Close
					</button>
				</div>
			{/if}
		</div>
	</section>

	<section class="panel lab-section">
		<h3>Progress (app-progress-track / app-progress-fill)</h3>
		<div class="lab-stack">
			{#each progressStops as stop (stop)}
				<div class="lab-progress-row">
					<code class="text-xs lab-progress-label">{stop}%</code>
					<div class="app-progress-track" style="flex: 1">
						<div class="app-progress-fill" style={`width: ${stop}%`}></div>
					</div>
				</div>
			{/each}
			<p class="text-xs muted-text">
				One track height for Status Panel, Work Center, and Remote Source. Status Panel layers
				its shimmer fill locally on the same primitive.
			</p>
		</div>
	</section>

	<section class="panel lab-section">
		<h3>Cover thumbnail (app-cover-thumb)</h3>
		<div class="lab-row">
			<div class="app-cover-thumb"><span>No Art</span></div>
			<div class="app-cover-thumb"><span>Loading…</span></div>
			<div class="app-cover-thumb" style="--cover-thumb-size: 6rem"><span>6rem via token</span></div>
		</div>
	</section>

	<section class="panel lab-section">
		<h3>Modal status and empty state</h3>
		<div class="lab-stack">
			<div class="app-modal-status text-xs">Neutral status message.</div>
			<div class="app-modal-status is-error text-xs">Something failed — error variant.</div>
			<div class="app-modal-status is-success text-xs">Applied — success variant.</div>
			<div class="app-modal-empty">
				<p>Empty state body copy teaching the surface.</p>
				<button class="btn-pill btn-pill-secondary mt-2" type="button">Suggested action</button>
			</div>
		</div>
	</section>

	<section class="panel lab-section">
		<h3>Density-driven rows</h3>
		<div class="lab-stack">
			{#each ['01 - Prologue.m4b', '02 - The First Step.m4b', '03 - Into the Unknown.m4b'] as name, i (name)}
				<div class="lab-density-row" class:lab-density-row-sel={i === 1}>
					<span>{name}</span>
					<span class="muted-text lab-mono">00:{18 + i}:45</span>
				</div>
			{/each}
			<p class="text-xs muted-text">
				Row height, padding, and text size read the density tokens — flip the switch above.
			</p>
		</div>
	</section>

	<section class="panel lab-section" data-testid="state-fixtures-section">
		<h3>State fixtures</h3>
		<div class="lab-state-grid">
			{#each stateFixtures as fixture (fixture.id)}
				<article
					class="lab-state-card"
					class:is-active={fixture.tone === 'active'}
					class:is-error={fixture.tone === 'error'}
					class:is-success={fixture.tone === 'success'}
					class:is-selected={fixture.tone === 'selected'}
					data-testid={`lab-state-${fixture.id}`}
				>
					<div class="lab-state-card-head">
						<strong>{fixture.label}</strong>
						<span class="text-xs muted-text">{fixture.meta}</span>
					</div>
					<p class="text-xs">{fixture.body}</p>
					<div class="app-progress-track" aria-label={`${fixture.label} progress`}>
						<div class="app-progress-fill" style={`width: ${fixture.progress}%`}></div>
					</div>
				</article>
			{/each}
		</div>
	</section>

	<section class="panel lab-section" data-testid="metadata-form-fixtures-section">
		<div class="lab-section-head">
			<div>
				<h3>Metadata form presets</h3>
				<p class="text-xs muted-text">{activeMetadataPresetDetails?.summary}</p>
			</div>
			<div class="lab-preset-controls" role="group" aria-label="Metadata form presets">
				{#each metadataFormLabPresets as preset (preset.id)}
					<button
						class="btn-pill"
						class:btn-pill-primary={activeMetadataPreset === preset.id}
						class:btn-pill-secondary={activeMetadataPreset !== preset.id}
						type="button"
						data-testid={`metadata-preset-${preset.id}`}
						aria-pressed={activeMetadataPreset === preset.id}
						onclick={() => setMetadataPreset(preset.id)}
					>
						{preset.label}
					</button>
				{/each}
			</div>
		</div>
		<div class="lab-metadata-fixture" data-testid={`metadata-fixture-${activeMetadataPreset}`}>
			<MetadataFormFieldsIsland
				onFieldInput={onMetadataFormFieldInput}
				onActionChange={onMetadataFormActionSelectChange}
				onSaveMetadata={() => {}}
			/>
		</div>
	</section>
</div>

<style>
	.lab {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		max-width: 60rem;
		margin: 0 auto;
		padding: var(--space-5);
		overflow: auto;
		height: 100%;
	}

	.lab-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--space-4);
	}

	.lab-density {
		display: flex;
		gap: var(--space-2);
		flex-shrink: 0;
	}

	.lab-section h3 {
		margin-bottom: var(--space-3);
	}

	.lab-section-head {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--space-3);
		margin-bottom: var(--space-3);
	}

	.lab-section-head h3 {
		margin-bottom: var(--space-1);
	}

	.lab-swatches {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(9.5rem, 1fr));
		gap: var(--space-2);
	}

	.lab-swatch {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
	}

	.lab-swatch-chip {
		width: 1.75rem;
		height: 1.75rem;
		flex-shrink: 0;
		border: 1px solid var(--border-secondary);
		border-radius: var(--radius-sm);
	}

	.lab-scale-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
		gap: var(--space-4);
	}

	.lab-space-row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		margin-top: var(--space-1);
	}

	.lab-space-bar {
		height: 0.5rem;
		background: var(--accent-primary);
		border-radius: var(--radius-sm);
	}

	.lab-radius-row {
		display: flex;
		gap: var(--space-2);
		margin-top: var(--space-2);
		flex-wrap: wrap;
	}

	.lab-radius-chip {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 4.5rem;
		height: 3rem;
		border: 1px solid var(--border-secondary);
		background: var(--bg-input);
	}

	.lab-row {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		flex-wrap: wrap;
	}

	.lab-popover-stage {
		position: relative;
		min-height: calc(9rem + var(--density-row-h));
		padding: var(--density-pad);
		border: 1px dashed var(--border-secondary);
		border-radius: var(--radius-md);
	}

	.lab-popover {
		width: 18rem;
		padding: var(--density-pad);
		font-size: var(--density-text);
	}

	.lab-popover p {
		margin: var(--space-2) 0;
	}

	.lab-stack {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.lab-progress-row {
		display: flex;
		align-items: center;
		gap: var(--space-3);
	}

	.lab-progress-label {
		width: 2.5rem;
		text-align: right;
	}

	.lab-density-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: var(--density-row-h);
		padding: 0 var(--density-pad);
		font-size: var(--density-text);
		border: 1px solid var(--border-primary);
		border-radius: var(--radius-md);
	}

	.lab-density-row-sel {
		border-color: var(--border-focus);
		background: color-mix(in srgb, var(--accent-primary) 12%, transparent);
	}

	.lab-mono {
		font-family: var(--font-mono);
		font-size: var(--text-sm);
	}

	.lab-state-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr));
		gap: var(--space-2);
	}

	.lab-state-card {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		min-height: 7rem;
		padding: var(--space-3);
		border: 1px solid var(--border-primary);
		border-radius: var(--radius-md);
		background: var(--bg-input);
	}

	.lab-state-card-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-2);
	}

	.lab-state-card.is-active {
		border-color: var(--accent-primary);
	}

	.lab-state-card.is-error {
		border-color: var(--text-error);
	}

	.lab-state-card.is-success {
		border-color: var(--text-success);
	}

	.lab-state-card.is-selected {
		border-color: var(--border-focus);
		background: color-mix(in srgb, var(--accent-primary) 10%, var(--bg-input));
	}

	.lab-preset-controls {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: var(--space-2);
	}

	.lab-metadata-fixture {
		padding: var(--space-3);
		border: 1px solid var(--border-secondary);
		border-radius: var(--radius-md);
		background: var(--bg-panel);
	}
</style>
