import { createSignal, For } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { Button, CoverThumb, Dialog, Progress } from '../ui/foundation';
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
					<p class="muted-text">
						Dev-only rendering surface for tokens and primitives. Not part of the app build. Open at
						/lab.html on the Vite dev server.
					</p>
				</div>
				<fieldset class="lab-density">
					<legend>Density</legend>
					<Button
						tone={density() === 'comfortable' ? 'primary' : 'secondary'}
						onClick={() => setDensity('comfortable')}
					>
						Comfortable
					</Button>
					<Button
						tone={density() === 'compact' ? 'primary' : 'secondary'}
						onClick={() => setDensity('compact')}
					>
						Compact
					</Button>
				</fieldset>
			</header>

			<section class="panel lab-section">
				<h3>Color tokens</h3>
				<div class="lab-swatches">
					<For each={colorTokens}>
						{(token) => (
							<div class="lab-swatch">
								<div class="lab-swatch-chip" style={{ background: `var(${token})` }} />
								<code>{token}</code>
							</div>
						)}
					</For>
				</div>
			</section>

			<section class="panel lab-section">
				<h3>Scale tokens</h3>
				<div class="lab-scale-grid">
					<div>
						<h4 class="muted-text">Spacing</h4>
						<For each={spaceTokens}>
							{(token) => (
								<div class="lab-space-row">
									<code>{token}</code>
									<div class="lab-space-bar" style={{ width: `var(${token})` }} />
								</div>
							)}
						</For>
					</div>
					<div>
						<h4 class="muted-text">Type</h4>
						<For each={typeTokens}>
							{(token) => (
								<div style={{ 'font-size': `var(${token})` }}>
									Aa — the quick brown fox <code>{token}</code>
								</div>
							)}
						</For>
					</div>
					<div>
						<h4 class="muted-text">Radius</h4>
						<div class="lab-radius-row">
							<For each={radiusTokens}>
								{(token) => (
									<div class="lab-radius-chip" style={{ 'border-radius': `var(${token})` }}>
										<code>{token.replace('--radius-', '')}</code>
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
					<Button tone="primary">Primary</Button>
					<Button>Secondary</Button>
					<Button disabled>Disabled</Button>
					<label class="checkbox-label tight">
						<input type="checkbox" checked />
						<span class="option-label">Checkbox label</span>
					</label>
					<input type="text" placeholder="Text input" class="lab-control" />
					<select class="lab-control">
						<option>Select option</option>
					</select>
				</div>
			</section>

			<section class="panel lab-section">
				<h3>Progress</h3>
				<div class="lab-stack">
					<For each={progressStops}>
						{(stop) => (
							<div class="lab-progress-row">
								<code class="lab-progress-label">{stop}%</code>
								<Progress value={stop} class="lab-progress" />
							</div>
						)}
					</For>
					<p class="muted-text">
						One track height for Status Panel, Work Center, and Remote Source. Status Panel layers
						its shimmer fill locally on the same primitive.
					</p>
				</div>
			</section>

			<section class="panel lab-section">
				<h3>Cover thumbnail</h3>
				<div class="lab-row">
					<CoverThumb>
						<span>No Art</span>
					</CoverThumb>
					<CoverThumb>
						<span>Loading…</span>
					</CoverThumb>
					<CoverThumb class="lab-cover-large">
						<span>6rem via token</span>
					</CoverThumb>
				</div>
			</section>

			<section class="panel lab-section">
				<h3>Dialog status and empty state</h3>
				<div class="lab-stack">
					<Dialog.Status>Neutral status message.</Dialog.Status>
					<Dialog.Status tone="error">Something failed — error variant.</Dialog.Status>
					<Dialog.Status tone="success">Applied — success variant.</Dialog.Status>
					<div class="app-modal-empty">
						<p>Empty state body copy teaching the surface.</p>
						<Button class="lab-empty-action">Suggested action</Button>
					</div>
				</div>
			</section>

			<section class="panel lab-section">
				<h3>Density-driven rows</h3>
				<div class="lab-stack">
					<For each={densityRows}>
						{(name, index) => (
							<div class={['lab-density-row', { 'lab-density-row-sel': index() === 1 }]}>
								<span>{name}</span>
								<span class="muted-text lab-mono">00:{18 + index()}:45</span>
							</div>
						)}
					</For>
					<p class="muted-text">
						Row height, padding, and text size read the density tokens. Flip the switch above.
					</p>
				</div>
			</section>
		</div>
	);
}
