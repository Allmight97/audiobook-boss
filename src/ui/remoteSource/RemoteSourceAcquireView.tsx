import { createEffect, createSignal, For, onCleanup, onMount, Show, type JSX } from 'solid-js';
import {
	bytesLabel,
	cancelRemoteSourceCoverPreviewSchedule,
	closeRemoteSourceAcquireAtom,
	getRemoteSourceCoverPreviewState,
	isAcquisitionTerminal,
	isTitleAcquirable,
	patchRemoteSourceViewAtom,
	progressPercent,
	progressTitleLabel,
	remoteSourceViewAtom,
	runRemoteSourceActionAtom,
	scheduleRemoteSourceCoverPreviews,
	selectedRemoteTitleSummaryText,
	subscribeRemoteSourceCoverPreviews,
	titleAvailability,
	toggledRemoteTitleSelection,
	toggledSupplementalPdfPreference,
	visibleRemoteTitles,
} from '../../app/remoteSource';
import { useAtomSet, useAtomValue } from '../../app/runtime/solid';
import { tauriClient } from '../../lib/tauri/client';
import { Dialog } from '../../lib/ui/Dialog';
import type { RemoteTitle } from '../../types/remoteSource';
import './remoteSourceAcquire.css';

function RemoteTitleCover(props: {
	readonly title: RemoteTitle;
	readonly revision: number;
}): JSX.Element {
	const previewState = () => {
		props.revision;
		return getRemoteSourceCoverPreviewState(props.title.coverUrl);
	};
	const readyUrl = () => {
		const state = previewState();
		return state.status === 'ready' ? state.dataUrl : '';
	};

	return (
		<div
			class="app-cover-thumb remote-title-cover"
			role="presentation"
			data-testid="remote-title-cover"
		>
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
		</div>
	);
}

export function RemoteSourceAcquireView(): JSX.Element {
	const view = useAtomValue(() => remoteSourceViewAtom);
	const runAction = useAtomSet(() => runRemoteSourceActionAtom);
	const close = useAtomSet(() => closeRemoteSourceAcquireAtom);
	const patchView = useAtomSet(() => patchRemoteSourceViewAtom);
	const [previewRevision, setPreviewRevision] = createSignal(0);

	onMount(() => {
		onCleanup(subscribeRemoteSourceCoverPreviews(() => setPreviewRevision((value) => value + 1)));
	});

	createEffect(() => {
		const current = view();
		if (current.isOpen && !current.didHydrateOpenDialog) {
			patchView({ didHydrateOpenDialog: true });
			void runAction({ type: 'hydrateOpenDialog' });
		}
	});

	createEffect(() => {
		const current = view();
		if (!current.isOpen || current.accountState?.status !== 'connected') {
			cancelRemoteSourceCoverPreviewSchedule();
			return;
		}
		const visible = visibleRemoteTitles(current.titles, {
			titleFilter: current.titleFilter,
			showSupplementalPdfOnly: current.showSupplementalPdfOnly,
			hideUnavailableTitles: current.hideUnavailableTitles,
		});
		scheduleRemoteSourceCoverPreviews(
			visible.map((title) => title.coverUrl),
			(url) => tauriClient.loadCoverArtFromUrl(url),
		);
		onCleanup(() => cancelRemoteSourceCoverPreviewSchedule());
	});

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
			onClose={() => close(undefined)}
			labelledBy="remote-source-title"
			testId="remote-source-modal"
		>
			<div class="app-modal-header">
				<h3 id="remote-source-title">Acquire Audiobooks</h3>
				<div class="remote-source-header-actions">
					<Show when={view().accountState?.status === 'connected'}>
						<button
							class="btn-pill btn-pill-secondary"
							disabled={view().isBusy}
							type="button"
							onClick={() => void runAction({ type: 'logout' })}
						>
							Logout
						</button>
					</Show>
					<button
						id="remote-source-close"
						class="btn-pill btn-pill-secondary"
						data-testid="remote-source-close"
						type="button"
						onClick={() => close(undefined)}
					>
						Close
					</button>
				</div>
			</div>

			<div class="app-modal-body">
				<div class="app-modal-controls">
					<div class="app-modal-field">
						<label for="remote-source-provider">Source</label>
						<select id="remote-source-provider" disabled>
							<option>Audible</option>
						</select>
					</div>

					<Show
						when={view().accountState?.status === 'connected'}
						fallback={
							<>
								<div class="app-modal-field app-modal-field-button">
									<button
										class="btn-pill btn-pill-primary"
										disabled={view().isBusy}
										type="button"
										onClick={() => void runAction({ type: 'startAuth' })}
									>
										Connect Audible
									</button>
								</div>
								<div class="app-modal-field remote-source-handoff">
									<label for="remote-source-handoff">Auth Handoff</label>
									<input
										id="remote-source-handoff"
										type="text"
										placeholder="Final Amazon URL or handoff file path"
										value={view().handoffPath}
										onInput={(event) => patchView({ handoffPath: event.currentTarget.value })}
									/>
								</div>
								<div class="app-modal-field app-modal-field-button">
									<button
										class="btn-pill btn-pill-secondary"
										disabled={view().isBusy}
										type="button"
										onClick={() => void runAction({ type: 'completeAuth' })}
									>
										Complete Auth
									</button>
								</div>
							</>
						}
					>
						<div class="app-modal-field app-modal-field-button">
							<button
								class="btn-pill btn-pill-secondary"
								disabled={view().isBusy}
								type="button"
								onClick={() => void runAction({ type: 'loadLibrary' })}
							>
								Refresh Library
							</button>
						</div>
						<div class="app-modal-field app-modal-field-button">
							<button
								class="btn-pill btn-pill-primary"
								disabled={view().isBusy || view().selectedTitleIds.size === 0}
								type="button"
								onClick={() => void runAction({ type: 'acquireSelected' })}
							>
								Acquire Selected
							</button>
						</div>
						<div class="app-modal-field remote-source-filter">
							<label for="remote-source-filter">Filter</label>
							<input
								id="remote-source-filter"
								type="search"
								placeholder="Filter loaded titles"
								value={view().titleFilter}
								onInput={(event) => patchView({ titleFilter: event.currentTarget.value })}
							/>
						</div>
						<div class="app-modal-field app-modal-field-toggle remote-source-pdf-filter">
							<label class="checkbox-label text-xs mb-0">
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
						<div class="app-modal-field app-modal-field-toggle remote-source-availability-filter">
							<label class="checkbox-label text-xs mb-0">
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
					<div class="remote-source-status app-modal-status text-xs" aria-live="polite">
						{view().statusMessage}
					</div>
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
									<div
										class="app-progress-track"
										role="progressbar"
										aria-label="Acquisition progress"
										aria-valuemin={0}
										aria-valuemax={100}
										aria-valuenow={Math.round(progressPercent(job()))}
									>
										<span
											class="app-progress-fill"
											style={{ width: `${progressPercent(job())}%` }}
										/>
									</div>
									<div class="remote-progress-copy">
										<span>{progressTitleLabel(progress(), view().titles)}</span>
										<span>{Math.round(progressPercent(job()))}%</span>
									</div>
									<Show when={bytesLabel(progress())}>
										{(label) => <p class="remote-progress-bytes">{label()}</p>}
									</Show>
									<Show when={!isAcquisitionTerminal(job())}>
										<button
											class="btn-pill btn-pill-secondary remote-progress-cancel"
											type="button"
											onClick={() => void runAction({ type: 'cancelActiveAcquisition' })}
										>
											Cancel Acquisition
										</button>
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
								<div
									class="remote-title-row"
									classList={{
										selected: view().selectedTitleIds.has(title.titleId),
										unavailable: !isTitleAcquirable(title),
									}}
									role="option"
									tabIndex={-1}
									aria-selected={view().selectedTitleIds.has(title.titleId)}
									aria-disabled={!isTitleAcquirable(title)}
								>
									<RemoteTitleCover title={title} revision={previewRevision()} />
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
			</div>
		</Dialog>
	);
}
