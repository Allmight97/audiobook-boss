import { createSignal, For } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { useAppRuntime } from '../../app/runtime';
import { compactRows, leftRows, rightRows } from './rows';
import './tagPreview.css';

export function TagPreviewView(props: { readonly variant?: 'full' | 'workbench' }): JSX.Element {
	const view = useAppRuntime().metadata.view;
	const [showAllTags, setShowAllTags] = createSignal(false);
	const tags = () => view().tags;
	const variant = () => props.variant ?? 'full';

	return variant() === 'workbench' ? (
		<div class="tag-preview-workbench">
			<section class="tag-compact-table" aria-label="Basic metadata tags">
				<For each={compactRows}>
					{(row) => (
						<div class="tag-compact-row" title={row.title}>
							<span class="tag-compact-name">{row.label}</span>
							<span
								class={['tag-compact-value', { 'tag-value-empty': !tags()[row.field] }]}
								data-field={row.field}
							>
								{tags()[row.field] || '—'}
							</span>
						</div>
					)}
				</For>
			</section>
			{showAllTags() && (
				<div class="tag-expanded" data-testid="all-tags-expanded">
					<div class="tag-grid">
						<div class="tag-column">
							<For each={leftRows}>
								{(row) => (
									<div class="tag-row" title={row.title}>
										<span class="tag-name">{row.label}</span>
										<span class="tag-value" data-field={`all-${row.field}`}>
											{tags()[row.field] || '—'}
										</span>
									</div>
								)}
							</For>
						</div>
						<div class="tag-column">
							<For each={rightRows}>
								{(row) => (
									<div class="tag-row" title={row.title}>
										<span class="tag-name">{row.label}</span>
										<span class="tag-value" data-field={`all-${row.field}`}>
											{tags()[row.field] || '—'}
										</span>
									</div>
								)}
							</For>
						</div>
					</div>
				</div>
			)}
			<button
				type="button"
				class="show-all-tags-button"
				data-testid="show-all-tags-button"
				aria-expanded={showAllTags() ? 'true' : 'false'}
				onClick={() => setShowAllTags((current) => !current)}
			>
				{showAllTags() ? 'Hide Extra Tags' : 'Show All Tags'}
			</button>
		</div>
	) : (
		<div class="tag-grid tag-grid-full">
			<div class="tag-column">
				<For each={leftRows}>
					{(row) => (
						<div class="tag-row" title={row.title}>
							<span class="tag-name">{row.label}</span>
							<span class="tag-value" data-field={row.field}>
								{tags()[row.field] || '—'}
							</span>
						</div>
					)}
				</For>
			</div>
			<div class="tag-column">
				<For each={rightRows}>
					{(row) => (
						<div class="tag-row" title={row.title}>
							<span class="tag-name">{row.label}</span>
							<span class="tag-value" data-field={row.field}>
								{tags()[row.field] || '—'}
							</span>
						</div>
					)}
				</For>
			</div>
		</div>
	);
}
