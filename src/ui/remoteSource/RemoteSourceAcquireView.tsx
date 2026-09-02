import { createEffect, For, Show } from 'solid-js';
import type { JSX } from '@solidjs/web';

import {
	bytesLabel,
	isAcquisitionTerminal,
	isTitleAcquirable,
	progressPercent,
	progressTitleLabel,
	selectedRemoteTitleSummaryText,
	titleAvailability,
	toggledRemoteTitleSelection,
	toggledSupplementalPdfPreference,
	visibleRemoteTitles,
} from '../../app/remoteSource';
import { useAppRuntime } from '../../app/runtime';
import { Button, CoverThumb, Dialog, Progress } from '../foundation';
import type { RemoteTitle } from '../../types/remoteSource';
import './remoteSourceAcquire.css';

function RemoteTitleCover(props: { readonly title: RemoteTitle }): JSX.Element {
	const coverPreview = useAppRuntime().remoteSource.coverPreview;
	const previewState = () => {
		return coverPreview(props.title.coverUrl);
	};
	const readyUrl = () => {
		const state = previewState();
		return state.status === 'ready' ? state.dataUrl : '';
	};

	return (
		<CoverThumb class="remote-title-cover" testId="remote-title-cover">
			<Show
				when={props.title.coverUrl}
				fallback={<span data-testid="remote-title-cover-missing">No Art</span>}
			>
				<Show
					when={previewState().status === 'ready'}
					fallback={
						previewState().status === 'loading' || previewState().status === 'queued' ? (
							<span data-testid="remote-title-cover-loading">Loading…</span>
						) : previewState().status === 'error' ? (
							<span data-testid="remote-title-cover-error">Preview failed</span>
						) : (
							<span data-testid="remote-title-cover-available">Art Available</span>
						)
					}
				>
					<img
						src={readyUrl()}
						alt={`${props.title.title} cover art`}
						data-testid="remote-title-cover-image"
					/>
				</Show>
			</Show>
		</CoverThumb>
	);
}

export function RemoteSourceAcquireView(): JSX.Element {
	const remoteSource = useAppRuntime().remoteSource;
	const view = remoteSource.view;
	const runAction = remoteSource.runAction;
	const close = remoteSource.close;
	const patchView = remoteSource.patch;

	createEffect(
		() => {
			const current = view();
			return current.isOpen && !current.didHydrateOpenDialog;
		},
		(shouldHydrate) => {
			if (shouldHydrate) {
				patchView({ didHydrateOpenDialog: true });
				void runAction({ type: 'hydrateOpenDialog' });
			}
		},
	);

	createEffect(
		() => {
			const current = view();
			return {
				isOpen: current.isOpen,
				connected: current.accountState?.status === 'connected',
				titles: current.titles,
				titleFilter: current.titleFilter,
				showSupplementalPdfOnly: current.showSupplementalPdfOnly,
				hideUnavailableTitles: current.hideUnavailableTitles,
			};
		},
		(current) => {
			if (!current.isOpen || !current.connected) {
				remoteSource.cancelCoverPreviews();
				return;
			}
			const visible = visibleRemoteTitles(current.titles, {
				titleFilter: current.titleFilter,
				showSupplementalPdfOnly: current.showSupplementalPdfOnly,
				hideUnavailableTitles: current.hideUnavailableTitles,
			});
			remoteSource.scheduleCoverPreviews(visible.map((title) => title.coverUrl));
			return () => remoteSource.cancelCoverPreviews();
		},
	);

	const visibleTitles = () =>
		visibleRemoteTitles(view().titles, {
			titleFilter: view().titleFilter,
			showSupplementalPdfOnly: view().showSupplementalPdfOnly,
			hideUnavailableTitles: view().hideUnavailableTitles,
		});

	return (
		<Dialog
			id="remote-source-modal"
			open={view().isOpen}
			onClose={() => close()}
			labelledBy="remote-source-title"
			testId="remote-source-modal"
		>
			<Dialog.Header>
				<h3 id="remote-source-title">Acquire Audiobooks</h3>
				<div class="remote-source-header-actions">
					<Show when={view().accountState?.status === 'connected'}>
						<Button disabled={view().isBusy} onClick={() => void runAction({ type: 'logout' })}>
							Logout
						</Button>
					</Show>
					<Button
						id="remote-source-close"
						data-testid="remote-source-close"
						onClick={() => close()}
					>
						Close
					</Button>
				</div>
			</Dialog.Header>

			<Dialog.Body>
				<div class="remote-source-toolbar">
					<div class="remote-source-toolbar-field remote-source-provider">
						<label for="remote-source-provider">Source</label>
						<select id="remote-source-provider" disabled>
							<option>Audible</option>
						</select>
					</div>

					<Show
						when={view().accountState?.status === 'connected'}
						fallback={
							<>
								<div class="remote-source-toolbar-field remote-source-toolbar-button">
									<Button
										tone="primary"
										disabled={view().isBusy}
										onClick={() => void runAction({ type: 'startAuth' })}
									>
										Connect Audible
									</Button>
								</div>
								<div class="remote-source-toolbar-field remote-source-handoff">
									<label for="remote-source-handoff">Auth Handoff</label>
									<input
										id="remote-source-handoff"
										type="text"
										placeholder="Final Amazon URL or handoff file path"
										value={view().handoffPath}
										onInput={(event) => patchView({ handoffPath: event.currentTarget.value })}
									/>
								</div>
								<div class="remote-source-toolbar-field remote-source-toolbar-button">
									<Button
										disabled={view().isBusy}
										onClick={() => void runAction({ type: 'completeAuth' })}
									>
										Complete Auth
									</Button>
								</div>
							</>
						}
					>
						<div class="remote-source-toolbar-field remote-source-toolbar-button">
							<Button
								disabled={view().isBusy}
								onClick={() => void runAction({ type: 'loadLibrary' })}
							>
								Refresh Library
							</Button>
						</div>
						<div class="remote-source-toolbar-field remote-source-toolbar-button">
							<Button
								tone="primary"
								disabled={view().isBusy || view().selectedTitleIds.size === 0}
								onClick={() => void runAction({ type: 'acquireSelected' })}
							>
								Acquire Selected
							</Button>
						</div>
						<div class="remote-source-toolbar-field remote-source-filter">
							<label for="remote-source-filter">Filter</label>
							<input
								id="remote-source-filter"
								type="search"
								placeholder="Filter loaded titles"
								value={view().titleFilter}
								onInput={(event) => patchView({ titleFilter: event.currentTarget.value })}
							/>
						</div>
						<div class="remote-source-toolbar-field remote-source-toolbar-toggle remote-source-pdf-filter">
							<label class="checkbox-label tight">
								<input
									type="checkbox"
									checked={view().showSupplementalPdfOnly}
									onChange={(event) =>
										patchView({ showSupplementalPdfOnly: event.currentTarget.checked })
									}
								/>
								<span class="option-label">Supplemental PDF only</span>
							</label>
						</div>
						<div class="remote-source-toolbar-field remote-source-toolbar-toggle remote-source-availability-filter">
							<label class="checkbox-label tight">
								<input
									type="checkbox"
									checked={view().hideUnavailableTitles}
									onChange={(event) =>
										patchView({ hideUnavailableTitles: event.currentTarget.checked })
									}
								/>
								<span class="option-label">Hide unavailable</span>
							</label>
						</div>
					</Show>
				</div>

				<Show when={view().statusMessage}>
					<Dialog.Status class="remote-source-status" live="polite">
						{view().statusMessage}
					</Dialog.Status>
				</Show>

				<Show when={view().accountState?.status === 'connected'}>
					<div class="remote-selection-summary" aria-live="polite">
						<span>{selectedRemoteTitleSummaryText(view().selectedTitleIds, visibleTitles())}</span>
						<Show when={view().selectedTitleIds.size > 0}>
							<button
								class="remote-clear-selection"
								type="button"
								disabled={view().isBusy}
								onClick={() => patchView({ selectedTitleIds: new Set() })}
							>
								Clear selection
							</button>
						</Show>
					</div>
				</Show>

				<Show when={view().activeJob}>
					{(job) => (
						<Show when={job().progress}>
							{(progress) => (
								<div class="remote-progress" role="status" aria-live="polite">
									<Progress value={progressPercent(job())} label="Acquisition progress" />
									<div class="remote-progress-copy">
										<span>{progressTitleLabel(progress(), view().titles)}</span>
										<span>{Math.round(progressPercent(job()))}%</span>
									</div>
									<Show when={bytesLabel(progress())}>
										{(label) => <p class="remote-progress-bytes">{label()}</p>}
									</Show>
									<Show when={!isAcquisitionTerminal(job())}>
										<Button
											class="remote-progress-cancel"
											onClick={() => void runAction({ type: 'cancelActiveAcquisition' })}
										>
											Cancel Acquisition
										</Button>
									</Show>
								</div>
							)}
						</Show>
					)}
				</Show>

				<Show when={view().accountState?.status === 'connected'}>
					<div
						class="app-modal-results remote-title-list"
						role="listbox"
						aria-label="Audible titles"
						aria-multiselectable="true"
					>
						<For each={visibleTitles()}>
							{(title) => (
								// biome-ignore lint/a11y/useFocusableInteractive: Solid 2 JSX types expose tabindex, not tabIndex
								<div
									class={[
										'remote-title-row',
										{
											selected: view().selectedTitleIds.has(title.titleId),
											unavailable: !isTitleAcquirable(title),
										},
									]}
									role="option"
									tabindex={-1}
									aria-selected={view().selectedTitleIds.has(title.titleId) ? 'true' : 'false'}
									aria-disabled={!isTitleAcquirable(title) ? 'true' : undefined}
								>
									<RemoteTitleCover title={title} />
									<button
										type="button"
										class="remote-title-button"
										disabled={!isTitleAcquirable(title)}
										onClick={() =>
											patchView({
												selectedTitleIds: toggledRemoteTitleSelection(
													view().selectedTitleIds,
													title,
												),
											})
										}
									>
										<span class="remote-title-name">{title.title}</span>
										<span class="remote-title-meta">{title.authors.join(', ')}</span>
										<Show when={!isTitleAcquirable(title)}>
											<span class="remote-title-availability">
												{titleAvailability(title).label}
											</span>
											<Show when={titleAvailability(title).detail}>
												{(detail) => (
													<span class="remote-title-availability-detail">{detail()}</span>
												)}
											</Show>
										</Show>
									</button>
									<Show when={title.supplementalPdfAvailable}>
										<label class="remote-pdf-toggle">
											<input
												type="checkbox"
												disabled={!isTitleAcquirable(title)}
												checked={view().includePdfByTitleId[title.titleId] ?? true}
												onChange={() =>
													patchView({
														includePdfByTitleId: toggledSupplementalPdfPreference(
															view().includePdfByTitleId,
															title.titleId,
														),
													})
												}
											/>
											PDF
										</label>
									</Show>
								</div>
							)}
						</For>
					</div>
				</Show>
			</Dialog.Body>
		</Dialog>
	);
}
