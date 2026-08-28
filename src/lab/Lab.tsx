import { createSignal, For, type JSX } from 'solid-js';
import './lab.css';

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
	'--border-primary',
	'--border-secondary',
	'--border-focus',
	'--accent-primary',
	'--accent-primary-hover',
	'--progress-bg',
	'--progress-fg',
] as const;
const spaceTokens = [
	'--space-1',
	'--space-2',
	'--space-3',
	'--space-4',
	'--space-5',
	'--space-6',
] as const;
const typeTokens = ['--text-xs', '--text-sm', '--text-md', '--text-lg', '--text-xl'] as const;
const radiusTokens = ['--radius-sm', '--radius-md', '--radius-lg', '--radius-pill'] as const;
const progressStops = [0, 37, 64, 100] as const;
const densityRows = [
	'01 - Prologue.m4b',
	'02 - The First Step.m4b',
	'03 - Into the Unknown.m4b',
] as const;

export function Lab(): JSX.Element {
	const [density, setDensityState] = createSignal<'comfortable' | 'compact'>('comfortable');

	function setDensity(next: 'comfortable' | 'compact'): void {
		setDensityState(next);
		if (next === 'compact') {
			document.documentElement.dataset.density = 'compact';
		} else {
			delete document.documentElement.dataset.density;
		}
	}

	return (
		<div class="lab">
			<header class="lab-header panel">
				<div>
					<h2>ABB design lab</h2>
					<p class="text-xs muted-text">
						Dev-only rendering surface for tokens and primitives (#412 slice 1). Not part of the app
						build — open at /lab.html on the Vite dev server.
					</p>
				</div>
				<fieldset class="lab-density">
					<legend>Density</legend>
					<button
						class="btn-pill"
						classList={{
							'btn-pill-primary': density() === 'comfortable',
							'btn-pill-secondary': density() !== 'comfortable',
						}}
						type="button"
						onClick={() => setDensity('comfortable')}
					>
						Comfortable
					</button>
					<button
						class="btn-pill"
						classList={{
							'btn-pill-primary': density() === 'compact',
							'btn-pill-secondary': density() !== 'compact',
						}}
						type="button"
						onClick={() => setDensity('compact')}
					>
						Compact
					</button>
				</fieldset>
			</header>

			<section class="panel lab-section">
				<h3>Color tokens</h3>
				<div class="lab-swatches">
					<For each={colorTokens}>
						{(token) => (
							<div class="lab-swatch">
								<div class="lab-swatch-chip" style={{ background: `var(${token})` }} />
								<code class="text-xs">{token}</code>
							</div>
						)}
					</For>
				</div>
			</section>

			<section class="panel lab-section">
				<h3>Scale tokens</h3>
				<div class="lab-scale-grid">
					<div>
						<h4 class="text-xs muted-text">Spacing</h4>
						<For each={spaceTokens}>
							{(token) => (
								<div class="lab-space-row">
									<code class="text-xs">{token}</code>
									<div class="lab-space-bar" style={{ width: `var(${token})` }} />
								</div>
							)}
						</For>
					</div>
					<div>
						<h4 class="text-xs muted-text">Type</h4>
						<For each={typeTokens}>
							{(token) => (
								<div style={{ 'font-size': `var(${token})` }}>
									Aa — the quick brown fox <code class="text-xs">{token}</code>
								</div>
							)}
						</For>
					</div>
					<div>
						<h4 class="text-xs muted-text">Radius</h4>
						<div class="lab-radius-row">
							<For each={radiusTokens}>
								{(token) => (
									<div class="lab-radius-chip" style={{ 'border-radius': `var(${token})` }}>
										<code class="text-xs">{token.replace('--radius-', '')}</code>
									</div>
								)}
							</For>
						</div>
					</div>
				</div>
			</section>

			<section class="panel lab-section">
				<h3>Buttons and form controls</h3>
				<div class="lab-row">
					<button class="btn-pill btn-pill-primary" type="button">
						Primary
					</button>
					<button class="btn-pill btn-pill-secondary" type="button">
						Secondary
					</button>
					<button class="btn-pill btn-pill-secondary" type="button" disabled>
						Disabled
					</button>
					<label class="checkbox-label text-xs mb-0">
						<input type="checkbox" checked />
						<span class="option-label">Checkbox label</span>
					</label>
					<input type="text" placeholder="Text input" style={{ 'max-width': '200px' }} />
					<select style={{ 'max-width': '200px' }}>
						<option>Select option</option>
					</select>
				</div>
			</section>

			<section class="panel lab-section">
				<h3>Progress (app-progress-track / app-progress-fill)</h3>
				<div class="lab-stack">
					<For each={progressStops}>
						{(stop) => (
							<div class="lab-progress-row">
								<code class="text-xs lab-progress-label">{stop}%</code>
								<div class="app-progress-track" style={{ flex: '1' }}>
									<div class="app-progress-fill" style={{ width: `${stop}%` }} />
								</div>
							</div>
						)}
					</For>
					<p class="text-xs muted-text">
						One track height for Status Panel, Work Center, and Remote Source. Status Panel layers
						its shimmer fill locally on the same primitive.
					</p>
				</div>
			</section>

			<section class="panel lab-section">
				<h3>Cover thumbnail (app-cover-thumb)</h3>
				<div class="lab-row">
					<div class="app-cover-thumb">
						<span>No Art</span>
					</div>
					<div class="app-cover-thumb">
						<span>Loading…</span>
					</div>
					<div class="app-cover-thumb" style={{ '--cover-thumb-size': '6rem' }}>
						<span>6rem via token</span>
					</div>
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
						<button class="btn-pill btn-pill-secondary mt-2" type="button">
							Suggested action
						</button>
					</div>
				</div>
			</section>

			<section class="panel lab-section">
				<h3>Density-driven rows</h3>
				<div class="lab-stack">
					<For each={densityRows}>
						{(name, index) => (
							<div class="lab-density-row" classList={{ 'lab-density-row-sel': index() === 1 }}>
								<span>{name}</span>
								<span class="muted-text lab-mono">00:{18 + index()}:45</span>
							</div>
						)}
					</For>
					<p class="text-xs muted-text">
						Row height, padding, and text size read the density tokens — flip the switch above.
					</p>
				</div>
			</section>
		</div>
	);
}
