import {
	createEffect,
	createMemo,
	createSignal,
	onCleanup,
	onMount,
	Show,
	untrack,
	type JSX,
} from 'solid-js';

import type { AcquisitionLane, AppSettings, PinnedDefaults } from '../../types/appSettings';
import { useAppRuntime } from '../../app/runtime';
import { Button, Dialog } from '../foundation';
import { readFdkAfterburner, subscribeEncoderPanel } from '../encoderPanel';
import './appSettingsDialog.css';

const RESET_CONFIRM_MS = 4000;

const INDEXER_CATEGORY_OPTIONS: ReadonlyArray<{ readonly id: number; readonly label: string }> = [
	{ id: 3030, label: 'Audiobooks (3030)' },
	{ id: 3000, label: 'Audio (3000)' },
];

function indexerCategorySummary(selected: readonly number[]): string {
	const labels = INDEXER_CATEGORY_OPTIONS.filter((option) => selected.includes(option.id)).map(
		(option) => option.label,
	);
	return labels.length > 0 ? labels.join(', ') : 'Audiobooks (3030)';
}

function toggledIndexerCategories(selected: readonly number[], id: number): number[] {
	const next = selected.includes(id) ? selected.filter((value) => value !== id) : [...selected, id];
	return next.length > 0 ? next : [3030];
}

function IndexerCategoryPicker(props: {
	readonly selected: readonly number[];
	readonly onChange: (ids: number[]) => void;
}): JSX.Element {
	const [open, setOpen] = createSignal(false);
	let root: HTMLDivElement | undefined;

	onMount(() => {
		function handleWindowClick(event: MouseEvent): void {
			if (!open()) return;
			const target = event.target;
			if (target instanceof Node && root?.contains(target)) return;
			setOpen(false);
		}
		window.addEventListener('click', handleWindowClick);
		onCleanup(() => window.removeEventListener('click', handleWindowClick));
	});

	return (
		<div class="app-settings-category-picker" ref={root}>
			<button
				id="app-settings-indexer-category"
				type="button"
				class="app-settings-path-input app-settings-category-summary"
				data-testid="app-settings-indexer-category"
				aria-haspopup="listbox"
				aria-expanded={open() ? 'true' : 'false'}
				onClick={() => setOpen(!open())}
			>
				<span>{indexerCategorySummary(props.selected)}</span>
				<span class="app-settings-category-caret" aria-hidden="true">
					▼
				</span>
			</button>
			<div
				class={`app-settings-category-menu${open() ? ' open' : ''}`}
				role="listbox"
				aria-multiselectable="true"
			>
				{INDEXER_CATEGORY_OPTIONS.map((option) => (
					<label class="app-settings-category-option">
						<input
							type="checkbox"
							checked={props.selected.includes(option.id)}
							onChange={() => props.onChange(toggledIndexerCategories(props.selected, option.id))}
						/>
						{option.label}
					</label>
				))}
			</div>
		</div>
	);
}

function formatConcurrency(settings: Pick<AppSettings, 'maxConcurrentJobs'>): string {
	const preference = settings.maxConcurrentJobs;
	return preference.mode === 'fixed' ? `Fixed (${preference.value})` : 'Auto';
}

function formatEncoderType(encoderType: string): string {
	switch (encoderType) {
		case 'auto':
			return 'Auto';
		case 'fdk_he_aac':
			return 'FDK HE-AAC';
		case 'aac_at':
			return 'Apple AAC';
		case 'native_aac':
			return 'Native AAC';
		default:
			return encoderType;
	}
}

function formatEncoderDefaults(settings: Pick<AppSettings, 'encoderDefaults'>): string {
	const encoder = settings.encoderDefaults.settings;
	const mode =
		encoder.bitrateMode.mode === 'vbr'
			? `VBR ${encoder.bitrateMode.value}`
			: encoder.bitrateMode.mode.toUpperCase();
	return `${formatEncoderType(encoder.encoderType)} • ${encoder.bitrateKbps} kbps • ${mode}`;
}

function formatOutputDefaults(settings: Pick<AppSettings, 'outputDefaults'>): string {
	return settings.outputDefaults.outputDirectory ?? 'Not set';
}

function formatFdkSource(source: string): string {
	switch (source) {
		case 'user_configured':
			return 'user-configured path';
		case 'detected':
			return 'auto-detected';
		case 'bundled':
			return 'bundled';
		default:
			return 'unavailable';
	}
}

export function AppSettingsDialogView(): JSX.Element {
	const runtime = useAppRuntime();
	const settings = runtime.settings;
	const remoteSource = runtime.remoteSource;
	const state = settings.dialog;
	const isOpen = createMemo(() => state().isOpen);
	const indexerConnection = remoteSource.indexerConnection;
	const [resetConfirming, setResetConfirming] = createSignal(false);
	const [afterburnerRevision, setAfterburnerRevision] = createSignal(0);
	let resetConfirmTimeout: ReturnType<typeof setTimeout> | undefined;

	function cancelResetConfirm(): void {
		clearTimeout(resetConfirmTimeout);
		resetConfirmTimeout = undefined;
		setResetConfirming(false);
	}

	function requestResetConfirm(): void {
		setResetConfirming(true);
		clearTimeout(resetConfirmTimeout);
		resetConfirmTimeout = setTimeout(cancelResetConfirm, RESET_CONFIRM_MS);
	}

	function confirmReset(): void {
		cancelResetConfirm();
		void settings.resetAllAppSettings();
	}

	function handleWindowClickForResetConfirm(event: MouseEvent): void {
		if (!resetConfirming()) return;
		const target = event.target;
		if (target instanceof Element && target.closest('[data-testid="app-settings-reset-row"]')) {
			return;
		}
		cancelResetConfirm();
	}

	createEffect(() => {
		if (!isOpen()) {
			cancelResetConfirm();
			return;
		}
		untrack(() => {
			void remoteSource.loadIndexerConnectionSettings();
		});
	});

	onMount(() => {
		const unsubscribeEncoder = subscribeEncoderPanel(() => {
			setAfterburnerRevision((value) => value + 1);
		});
		window.addEventListener('click', handleWindowClickForResetConfirm, true);
		onCleanup(() => {
			unsubscribeEncoder();
			window.removeEventListener('click', handleWindowClickForResetConfirm, true);
			cancelResetConfirm();
		});
	});

	const pinnedDefaults = (): PinnedDefaults | undefined => state().settings?.pinnedDefaults;
	const startupBehavior = () => state().settings?.startupBehavior ?? 'rememberLastState';
	const defaultAcquisitionLane = (): AcquisitionLane => settings.defaultAcquisitionLane();
	const afterburner = () => {
		afterburnerRevision();
		return readFdkAfterburner();
	};

	return (
		<Dialog
			id="app-settings-modal"
			open={state().isOpen}
			onClose={() => settings.closeDialog()}
			labelledBy="app-settings-title"
			testId="app-settings-modal"
		>
			<Dialog.Header>
				<h3 id="app-settings-title">App Settings</h3>
				<Button
					id="app-settings-close"
					data-testid="app-settings-close"
					onClick={() => settings.closeDialog()}
				>
					Close
				</Button>
			</Dialog.Header>
			<Dialog.Body>
				<Show when={!state().loading} fallback={<p class="muted-text">Loading settings…</p>}>
					<section class="app-settings-section">
						<h4 class="app-settings-section-title">External FFmpeg (FDK AAC)</h4>
						<p class="muted-text">
							Point AudioBook Boss at an FFmpeg binary built with libfdk_aac to unlock the FDK
							HE-AAC encoder. The path is validated before it is used.
						</p>
						<div class="app-settings-path-row">
							<input
								id="app-settings-ffmpeg-path"
								class="app-settings-path-input"
								data-testid="app-settings-ffmpeg-path"
								type="text"
								placeholder="/opt/homebrew/bin/ffmpeg"
								value={state().ffmpegPathDraft}
								onInput={(event) => settings.setFfmpegPathDraft(event.currentTarget.value)}
							/>
							<Button
								data-testid="app-settings-ffmpeg-browse"
								onClick={() => void settings.browseForFfmpegBinary()}
							>
								Browse…
							</Button>
							<Button
								data-testid="app-settings-ffmpeg-clear"
								onClick={() => settings.clearFfmpegPathDraft()}
							>
								Clear
							</Button>
							<Button
								tone="primary"
								data-testid="app-settings-ffmpeg-save"
								disabled={state().saveState === 'saving'}
								onClick={() => void settings.saveToolchainPreference()}
							>
								{state().saveState === 'saving' ? 'Saving…' : 'Save'}
							</Button>
						</div>
						<Show when={state().saveState === 'error'}>
							<p
								class="app-settings-status app-settings-status-error"
								data-testid="app-settings-error"
							>
								{state().saveError}
							</p>
						</Show>
						<Show when={state().encoderAvailability}>
							{(availability) => (
								<p class="app-settings-status" data-testid="app-settings-toolchain-status">
									{availability().statusMessage}
									<Show when={availability().fdkAvailable}>
										{` (FDK source: ${formatFdkSource(availability().fdkSource)})`}
									</Show>
								</p>
							)}
						</Show>
						<label class="checkbox-label" data-testid="app-settings-afterburner-toggle">
							<input
								type="checkbox"
								id="app-settings-afterburner"
								data-testid="app-settings-afterburner-checkbox"
								checked={afterburner()}
								onChange={(event) =>
									settings.setFdkAfterburner(Boolean(event.currentTarget.checked))
								}
							/>
							<span class="option-label">FDK Afterburner</span>
						</label>
						<p class="muted-text">
							Extra encoding effort for slightly higher quality on the FDK encoder. Leave on unless
							encode speed matters more than quality.
						</p>
					</section>
					<Show when={state().settings}>
						{(_) => (
							<>
								<section class="app-settings-section">
									<h4 class="app-settings-section-title">Import</h4>
									<p class="muted-text">
										Choose which source the Import button opens by default. Use the caret to pick
										the other source any time.
									</p>
									<div
										class="app-settings-startup-options"
										role="radiogroup"
										aria-label="Default acquisition lane"
									>
										<label class="app-settings-radio">
											<input
												type="radio"
												name="app-settings-default-acquisition-lane"
												value="audible"
												data-testid="app-settings-default-lane-audible"
												checked={defaultAcquisitionLane() === 'audible'}
												onChange={() => void settings.setDefaultAcquisitionLane('audible')}
											/>
											Audible
										</label>
										<label class="app-settings-radio">
											<input
												type="radio"
												name="app-settings-default-acquisition-lane"
												value="indexer"
												data-testid="app-settings-default-lane-indexer"
												checked={defaultAcquisitionLane() === 'indexer'}
												onChange={() => void settings.setDefaultAcquisitionLane('indexer')}
											/>
											Indexer
										</label>
									</div>
								</section>
								<section class="app-settings-section">
									<h4 class="app-settings-section-title">Indexer connection</h4>
									<p class="muted-text">
										Configure your Indexer URL, API key, and audiobook categories. The API key is
										stored securely and is never shown again after save.
									</p>
									<div class="app-settings-path-row">
										<label class="app-settings-field-label" for="app-settings-indexer-url">
											URL
										</label>
										<input
											id="app-settings-indexer-url"
											class="app-settings-path-input"
											data-testid="app-settings-indexer-url"
											type="text"
											placeholder="http://192.168.0.20:9696"
											value={indexerConnection().baseUrlDraft}
											onInput={(event) =>
												remoteSource.patchIndexerConnectionSettings({
													baseUrlDraft: event.currentTarget.value,
												})
											}
										/>
									</div>
									<div class="app-settings-path-row">
										<label class="app-settings-field-label" for="app-settings-indexer-category">
											Categories
										</label>
										<IndexerCategoryPicker
											selected={indexerConnection().categoryIdsDraft}
											onChange={(categoryIdsDraft) =>
												remoteSource.patchIndexerConnectionSettings({ categoryIdsDraft })
											}
										/>
									</div>
									<div class="app-settings-path-row">
										<label class="app-settings-field-label" for="app-settings-indexer-api-key">
											API key
										</label>
										<input
											id="app-settings-indexer-api-key"
											class="app-settings-path-input"
											data-testid="app-settings-indexer-api-key"
											type="password"
											placeholder={
												indexerConnection().apiKeyConfigured
													? 'Replace stored API key'
													: 'Enter API key'
											}
											value={indexerConnection().apiKeyDraft}
											onInput={(event) =>
												remoteSource.patchIndexerConnectionSettings({
													apiKeyDraft: event.currentTarget.value,
												})
											}
										/>
									</div>
									<div class="app-settings-path-row">
										<Button
											tone="primary"
											data-testid="app-settings-indexer-save"
											disabled={indexerConnection().saveState === 'saving'}
											onClick={() => void remoteSource.saveIndexerConnectionSettings()}
										>
											{indexerConnection().saveState === 'saving' ? 'Saving…' : 'Save'}
										</Button>
										<Button
											data-testid="app-settings-indexer-test"
											disabled={indexerConnection().testState === 'testing'}
											onClick={() => void remoteSource.testIndexerConnection()}
										>
											{indexerConnection().testState === 'testing' ? 'Testing…' : 'Test'}
										</Button>
									</div>
									<Show when={indexerConnection().saveState === 'error'}>
										<p
											class="app-settings-status app-settings-status-error"
											data-testid="app-settings-indexer-save-error"
										>
											{indexerConnection().saveError}
										</p>
									</Show>
									<Show
										when={
											indexerConnection().testState === 'success' ||
											indexerConnection().testState === 'error'
										}
									>
										<p
											class={`app-settings-status${indexerConnection().testState === 'error' ? ' app-settings-status-error' : ''}`}
											data-testid="app-settings-indexer-test-status"
										>
											{indexerConnection().testMessage}
										</p>
									</Show>
									<Show when={indexerConnection().apiKeyConfigured}>
										<p class="muted-text" data-testid="app-settings-indexer-key-configured">
											An API key is configured. Enter a new key only to replace it.
										</p>
									</Show>
								</section>
								<section class="app-settings-section">
									<h4 class="app-settings-section-title">Startup settings</h4>
									<p class="muted-text">
										The encoder, output, and job controls save as you change them. This chooses what
										the app restores on launch.
									</p>
									<div
										class="app-settings-startup-options"
										role="radiogroup"
										aria-label="On launch"
									>
										<label class="app-settings-radio">
											<input
												type="radio"
												name="app-settings-startup-behavior"
												value="rememberLastState"
												data-testid="app-settings-startup-last"
												checked={startupBehavior() === 'rememberLastState'}
												onChange={() => void settings.setStartupBehavior('rememberLastState')}
											/>
											Remember my last settings
										</label>
										<label class="app-settings-radio">
											<input
												type="radio"
												name="app-settings-startup-behavior"
												value="pinnedDefaults"
												data-testid="app-settings-startup-pinned"
												checked={startupBehavior() === 'pinnedDefaults'}
												disabled={!pinnedDefaults()}
												onChange={() => void settings.setStartupBehavior('pinnedDefaults')}
											/>
											Use my pinned defaults
											<Show when={!pinnedDefaults()}>
												<span class="muted-text">(pin defaults first)</span>
											</Show>
										</label>
									</div>
									<div class="app-settings-path-row">
										<Button
											data-testid="app-settings-pin-defaults"
											disabled={state().startupSaveState === 'saving'}
											onClick={() => void settings.saveCurrentSettingsAsPinnedDefaults()}
										>
											Use current settings as defaults
										</Button>
									</div>
									<Show when={state().startupSaveState === 'error'}>
										<p
											class="app-settings-status app-settings-status-error"
											data-testid="app-settings-startup-error"
										>
											{state().startupSaveError}
										</p>
									</Show>
									<dl class="app-settings-summary" data-testid="app-settings-summary">
										<Show
											when={pinnedDefaults()}
											fallback={
												<>
													<dt>Pinned defaults</dt>
													<dd data-testid="app-settings-no-pin">Not pinned yet</dd>
												</>
											}
										>
											{(pinned) => (
												<>
													<dt>Pinned max jobs</dt>
													<dd>{formatConcurrency(pinned())}</dd>
													<dt>Pinned encoder</dt>
													<dd>{formatEncoderDefaults(pinned())}</dd>
													<dt>Pinned output folder</dt>
													<dd>{formatOutputDefaults(pinned())}</dd>
												</>
											)}
										</Show>
									</dl>
								</section>
								<section class="app-settings-section">
									<h4 class="app-settings-section-title">Reset</h4>
									<div class="app-settings-path-row" data-testid="app-settings-reset-row">
										<Show
											when={resetConfirming()}
											fallback={
												<Button
													data-testid="app-settings-reset"
													disabled={state().saveState === 'saving'}
													onClick={requestResetConfirm}
												>
													Reset all settings to defaults
												</Button>
											}
										>
											<span class="muted-text" data-testid="app-settings-reset-confirm-prompt">
												Reset all settings?
											</span>
											<Button
												data-testid="app-settings-reset-confirm"
												disabled={state().saveState === 'saving'}
												onClick={confirmReset}
											>
												Reset
											</Button>
											<Button data-testid="app-settings-reset-cancel" onClick={cancelResetConfirm}>
												Cancel
											</Button>
										</Show>
									</div>
								</section>
							</>
						)}
					</Show>
				</Show>
			</Dialog.Body>
		</Dialog>
	);
}
