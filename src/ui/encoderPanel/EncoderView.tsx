import { For, Show, createMemo, createUniqueId } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { useAppRuntime } from '../../app/runtime';
import type { AudioFile } from '../../types/audio';
import type { EncodingField } from '../../app/encoding';
import './encoderView.css';

export function EncoderView(
	props: { title?: AudioFile; titles?: readonly AudioFile[] } = {},
): JSX.Element {
	const runtime = useAppRuntime();
	const view = createMemo(() =>
		props.titles
			? runtime.encoding.selectionView(props.titles)
			: {
					...(props.title ? runtime.encoding.titleView(props.title) : runtime.encoding.view()),
					mixedFields: [] as readonly EncodingField[],
				},
	);
	const mixed = (field: EncodingField) => view().mixedFields.includes(field);
	const value = (field: EncodingField, current: string | number) => (mixed(field) ? '' : current);
	const editingTitles = () => !!props.title || !!props.titles;
	const instanceId = createUniqueId();
	const id = (name: string) => (editingTitles() ? `${name}-${instanceId}` : name);
	const choose = (field: EncodingField, selected: string) =>
		props.titles
			? runtime.encoding.selectTitles(props.titles, field, selected)
			: props.title
				? runtime.encoding.selectTitle(props.title, field, selected)
				: runtime.encoding.select(field, selected);

	function bind(field: EncodingField) {
		return (event: Event) => {
			const select = event.currentTarget as HTMLSelectElement | HTMLInputElement;
			choose(field, select.value);
			if (field === 'encoder') select.value = view().flavor;
			if (field === 'bitrate') select.value = String(view().bitrate);
		};
	}

	return (
		<div
			id={id('encoder-settings-panel')}
			class="encoder-workbench-panel"
			data-testid="encoder-settings-panel"
		>
			<div class="encoder-workbench-grid">
				<div class="encoder-field-row">
					<label for={id('audio-format')}>Output</label>
					<select
						id={id('audio-format')}
						value={value('format', view().format)}
						onChange={bind('format')}
					>
						<Show when={mixed('format')}>
							<option value="" disabled>
								Mixed
							</option>
						</Show>
						<option value="m4b">M4B · AAC</option>
						<option value="mp3">MP3</option>
						<option value="m4aOpus">M4A · Opus</option>
						<option value="mkaOpus">MKA · Opus</option>
					</select>
				</div>
				<div class="encoder-field-row">
					<label for={id('audio-intent')}>Audio handling</label>
					<select
						id={id('audio-intent')}
						value={value('intent', view().intent)}
						onChange={bind('intent')}
					>
						<Show when={mixed('intent')}>
							<option value="" disabled>
								Mixed
							</option>
						</Show>
						<option value="auto">Recommended</option>
						<option value="preserve">Keep original audio</option>
						<option value="encode">Use encoding settings</option>
					</select>
				</div>
				<Show when={mixed('format')}>
					<p class="field-hint">Choose one output format to edit encoding settings together.</p>
				</Show>
				<Show when={!mixed('format') && view().format !== 'mp3'}>
					<details class="encoder-conversion" open={!editingTitles()}>
						<summary>
							Encoding settings
							<Show when={!editingTitles() || view().intent === 'encode'}>
								{' '}
								·{' '}
								{mixed('encoder')
									? 'Mixed encoders'
									: mixed('quality') || mixed('bitrate')
										? 'Mixed quality'
										: view().flavor === 'opus'
											? `Opus · ${view().bitrate} kbps VBR`
											: `${view().flavorOptions.find((option) => option.value === view().flavor)?.label} · ${view().showQuality ? `VBR ${view().quality}` : `${view().bitrate} kbps`}`}
							</Show>
						</summary>
						<div class="encoder-workbench-grid">
							<div class="encoder-field-row">
								<label for={id('adv-encoder')}>Encoder</label>
								<div class="encoder-field-stack">
									<select
										id={id('adv-encoder')}
										data-testid="encoder-select"
										value={value('encoder', view().flavor)}
										disabled={view().flavorDisabled}
										onChange={bind('encoder')}
									>
										<Show when={mixed('encoder')}>
											<option value="" disabled>
												Mixed
											</option>
										</Show>
										<For each={view().flavorOptions}>
											{(option) => (
												<option value={option.value} disabled={option.disabled}>
													{option.label}
												</option>
											)}
										</For>
									</select>
									<p
										id={id('encoder-availability-hint')}
										class="field-hint"
										data-testid="encoder-availability-hint"
									>
										{view().availabilityHint}
										<Show when={view().fdkSetupNeeded}>
											{' '}
											<button
												type="button"
												class="encoder-setup-link"
												onClick={() => void runtime.settings.openDialog()}
											>
												Set up FDK…
											</button>
										</Show>
									</p>
								</div>
							</div>
							<Show when={mixed('encoder')}>
								<p class="field-hint">Choose one encoder to edit its settings together.</p>
							</Show>
							<Show when={!mixed('encoder')}>
								<Show
									when={view().faac}
									fallback={
										<div class="encoder-field-row">
											<span class="label">Profile</span>
											<div
												class="profile-display profile-display-workbench"
												data-testid="profile-display"
											>
												<span id={id('encoder-profile-display')}>{view().profileDisplay}</span>
												<span class="readonly-badge">read-only</span>
											</div>
										</div>
									}
								>
									<div class="encoder-field-row">
										<label for={id('faac-profile')}>Profile</label>
										<div class="encoder-field-stack">
											<select
												id={id('faac-profile')}
												value={value('faacProfile', view().faacProfile)}
												onChange={bind('faacProfile')}
												aria-describedby={id('faac-profile-hint')}
											>
												<Show when={mixed('faacProfile')}>
													<option value="" disabled>
														Mixed
													</option>
												</Show>
												<For each={view().faacProfileOptions}>
													{(option) => <option value={option.value}>{option.label}</option>}
												</For>
											</select>
											<p id={id('faac-profile-hint')} class="field-hint">
												{view().profileHint}
											</p>
										</div>
									</div>
									<div class="encoder-field-row">
										<label for={id('faac-rate-control')}>Rate control</label>
										<select
											id={id('faac-rate-control')}
											value={value('rateControl', view().rateControl)}
											onChange={bind('rateControl')}
										>
											<Show when={mixed('rateControl')}>
												<option value="" disabled>
													Mixed
												</option>
											</Show>
											<For each={view().rateControlOptions}>
												{(option) => <option value={option.value}>{option.label}</option>}
											</For>
										</select>
									</div>
								</Show>
								<Show when={!mixed('rateControl') && !mixed('faacProfile')}>
									<div class="encoder-field-row">
										<label
											for={id(view().showQuality ? 'output-quality' : 'output-bitrate')}
											id={id('quality-bitrate-label')}
										>
											{view().qualityBitrateLabel}
										</label>
										<div class="encoder-field-stack">
											<select
												id={id('output-quality')}
												hidden={!view().showQuality}
												data-testid="quality-select"
												aria-describedby={id('estimated-bitrate')}
												value={value('quality', view().quality)}
												onChange={bind('quality')}
											>
												<Show when={mixed('quality')}>
													<option value="" disabled>
														Mixed
													</option>
												</Show>
												<For each={view().qualityOptions}>
													{(option) => <option value={option.value}>{option.label}</option>}
												</For>
											</select>
											<input
												id={id('output-bitrate')}
												data-testid="bitrate-input"
												type="number"
												placeholder={mixed('bitrate') ? 'Mixed' : undefined}
												hidden={view().showQuality}
												min={view().bitrateKbpsMin}
												max={view().bitrateKbpsMax}
												step="1"
												value={value('bitrate', view().bitrate)}
												onChange={bind('bitrate')}
												aria-describedby={id('estimated-bitrate')}
											/>
											<p
												id={id('estimated-bitrate')}
												class="field-hint"
												data-testid="estimated-bitrate"
											>
												{view().estimatedBitrateText}
											</p>
										</div>
									</div>
								</Show>
								<div class="encoder-field-row">
									<label for={id('output-samplerate')}>Sample Rate</label>
									<div class="encoder-field-stack">
										<select
											id={id('output-samplerate')}
											data-testid="samplerate-select"
											value={value('sampleRate', view().sampleRate)}
											disabled={view().sampleRateDisabled}
											onChange={bind('sampleRate')}
										>
											<Show when={mixed('sampleRate')}>
												<option value="" disabled>
													Mixed
												</option>
											</Show>
											<For each={view().sampleRateOptions}>
												{(option) => (
													<option value={option.value} disabled={option.disabled}>
														{option.label}
													</option>
												)}
											</For>
										</select>
										<p
											id={id('output-samplerate-effective')}
											class="field-hint"
											data-testid="auto-samplerate-hint"
										>
											{view().sampleRateHint}
										</p>
									</div>
								</div>

								<div class="encoder-field-row">
									<label for={id('output-channels')}>Channels</label>
									<div class="encoder-field-stack">
										<select
											id={id('output-channels')}
											data-testid="channels-select"
											value={value('channels', view().channels)}
											disabled={view().channelsDisabled}
											onChange={bind('channels')}
										>
											<Show when={mixed('channels')}>
												<option value="" disabled>
													Mixed
												</option>
											</Show>
											<For each={view().channelOptions}>
												{(option) => <option value={option.value}>{option.label}</option>}
											</For>
										</select>
										<p
											id={id('output-channels-effective')}
											class="field-hint"
											data-testid="auto-channels-hint"
										>
											{view().channelsHint}
										</p>
									</div>
								</div>
								<Show when={view().native}>
									<details class="encoder-advanced">
										<summary>Advanced</summary>
										<div class="encoder-field-row">
											<label for={id('native-speed')}>NMR speed</label>
											<div class="encoder-field-stack">
												<select
													id={id('native-speed')}
													data-testid="native-speed-select"
													value={value('nativeSpeed', view().nativeSpeed)}
													onChange={bind('nativeSpeed')}
													aria-describedby={id('native-speed-hint')}
												>
													<Show when={mixed('nativeSpeed')}>
														<option value="" disabled>
															Mixed
														</option>
													</Show>
													<For each={view().nativeSpeedOptions}>
														{(option) => <option value={option.value}>{option.label}</option>}
													</For>
												</select>
												<p id={id('native-speed-hint')} class="field-hint">
													Higher values trade some quality for faster encoding.
												</p>
											</div>
										</div>
									</details>
								</Show>
							</Show>
						</div>
					</details>
				</Show>
			</div>
		</div>
	);
}
