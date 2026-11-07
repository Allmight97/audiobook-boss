import { For, Show, createMemo, createUniqueId, type Accessor } from 'solid-js';
import type { JSX } from '@solidjs/web';

import type { EncodingField, EncodingForm, EncodingScope, PanelView } from '../../app/encoding';
import { useAppRuntime } from '../../app/runtime';
import { Select } from '../foundation';
import './encoderView.css';

export function EncoderView(props: { scope: EncodingScope }): JSX.Element {
	const runtime = useAppRuntime();
	const form = createMemo(() => runtime.encoding.form(props.scope));
	const view = createMemo(() => form().view());
	return (
		<Show when={view()}>
			{(shown) => (
				<EncoderPanel form={form()} view={shown} open={props.scope.kind === 'defaults'} />
			)}
		</Show>
	);
}

function EncoderPanel(props: {
	form: EncodingForm;
	view: Accessor<PanelView>;
	open: boolean;
}): JSX.Element {
	const view = props.view;
	const form = () => props.form;
	const mixed = (field: EncodingField) => view().mixedFields.includes(field);
	const value = (field: EncodingField, current: string | number) => (mixed(field) ? '' : current);
	const displayedFlavor = () => view().effectiveFlavor;
	const instanceId = createUniqueId();
	const id = (name: string) => `${name}-${instanceId}`;
	const choose = (field: EncodingField) => (selected: string) => form().choose(field, selected);

	return (
		<div
			id={id('encoder-settings-panel')}
			class="encoder-workbench-panel"
			data-testid="encoder-settings-panel"
		>
			<Show when={form().refusal()}>
				<p class="field-hint" role="status" data-testid="audio-edit-refusal">
					{form().refusal()}
				</p>
			</Show>
			<div class="encoder-workbench-grid">
				<div class="encoder-field-row">
					<label for={id('audio-format')}>Output</label>
					<Select
						id={id('audio-format')}
						data-testid="audio-format-select"
						value={value('format', view().format)}
						onChoose={choose('format')}
					>
						<Show when={mixed('format')}>
							<option value="" disabled>
								Mixed
							</option>
						</Show>
						<For each={view().formatOptions} keyed={(option) => option.value}>
							{(option) => <option value={option().value}>{option().label}</option>}
						</For>
					</Select>
				</div>
				<div class="encoder-field-row">
					<label for={id('audio-intent')}>Audio handling</label>
					<Select
						id={id('audio-intent')}
						data-testid="audio-intent-select"
						value={value('intent', view().intent)}
						onChoose={choose('intent')}
					>
						<Show when={mixed('intent')}>
							<option value="" disabled>
								Mixed
							</option>
						</Show>
						<For each={view().intentOptions} keyed={(option) => option.value}>
							{(option) => <option value={option().value}>{option().label}</option>}
						</For>
					</Select>
				</div>
				<Show when={mixed('format')}>
					<p class="field-hint">Choose one output format to edit encoding settings together.</p>
				</Show>
				<Show when={!mixed('format') && !mixed('intent') && view().intent === 'encode'}>
					<details class="encoder-conversion" open={props.open}>
						<summary>Encoding settings · {form().summary()}</summary>
						<div class="encoder-workbench-grid">
							<div class="encoder-field-row">
								<label for={id('adv-encoder')}>Encoder</label>
								<div class="encoder-field-stack">
									<Select
										id={id('adv-encoder')}
										data-testid="encoder-select"
										value={value('encoder', displayedFlavor())}
										disabled={view().flavorDisabled}
										onChoose={choose('encoder')}
									>
										<Show when={mixed('encoder')}>
											<option value="" disabled>
												Mixed
											</option>
										</Show>
										<For each={view().flavorOptions} keyed={(option) => option.value}>
											{(option) => (
												<option value={option().value} disabled={option().disabled}>
													{option().label}
												</option>
											)}
										</For>
									</Select>
								</div>
							</div>
							<Show when={mixed('encoder')}>
								<p class="field-hint">Choose one encoder to edit its settings together.</p>
							</Show>
							<Show when={!mixed('encoder') && view().native}>
								<div class="encoder-field-row">
									<label for={id('native-speed')}>NMR speed</label>
									<div class="encoder-field-stack">
										<Select
											id={id('native-speed')}
											data-testid="native-speed-select"
											value={value('nativeSpeed', view().nativeSpeed)}
											onChoose={choose('nativeSpeed')}
											aria-describedby={id('native-speed-hint')}
										>
											<Show when={mixed('nativeSpeed')}>
												<option value="" disabled>
													Mixed
												</option>
											</Show>
											<For each={view().nativeSpeedOptions} keyed={(option) => option.value}>
												{(option) => <option value={option().value}>{option().label}</option>}
											</For>
										</Select>
										<p id={id('native-speed-hint')} class="field-hint">
											Higher values trade some quality for faster encoding.
										</p>
									</div>
								</div>
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
												<span class="profile-display-name">{view().profileDisplay}</span>
												<span class="readonly-badge">read-only</span>
											</div>
										</div>
									}
								>
									<div class="encoder-field-row">
										<label for={id('faac-profile')}>Profile</label>
										<div class="encoder-field-stack">
											<Select
												id={id('faac-profile')}
												data-testid="faac-profile-select"
												value={value('faacProfile', view().faacProfile)}
												onChoose={choose('faacProfile')}
											>
												<Show when={mixed('faacProfile')}>
													<option value="" disabled>
														Mixed
													</option>
												</Show>
												<For each={view().faacProfileOptions} keyed={(option) => option.value}>
													{(option) => <option value={option().value}>{option().label}</option>}
												</For>
											</Select>
										</div>
									</div>
									<div class="encoder-field-row">
										<label for={id('faac-rate-control')}>Rate control</label>
										<Select
											id={id('faac-rate-control')}
											data-testid="faac-rate-control-select"
											value={value('rateControl', view().rateControl)}
											onChoose={choose('rateControl')}
										>
											<Show when={mixed('rateControl')}>
												<option value="" disabled>
													Mixed
												</option>
											</Show>
											<For each={view().rateControlOptions} keyed={(option) => option.value}>
												{(option) => <option value={option().value}>{option().label}</option>}
											</For>
										</Select>
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
											<Select
												id={id('output-quality')}
												hidden={!view().showQuality}
												data-testid="quality-select"
												value={value('quality', view().quality)}
												onChoose={choose('quality')}
											>
												<Show when={mixed('quality')}>
													<option value="" disabled>
														Mixed
													</option>
												</Show>
												<For each={view().qualityOptions} keyed={(option) => option.value}>
													{(option) => <option value={option().value}>{option().label}</option>}
												</For>
											</Select>
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
												onChange={(event) => {
													const input = event.currentTarget;
													form().choose('bitrate', input.value);
													input.value = String(view().bitrate);
												}}
											/>
										</div>
									</div>
								</Show>
								<div class="encoder-field-row">
									<label for={id('output-samplerate')}>Sample Rate</label>
									<div class="encoder-field-stack">
										<Select
											id={id('output-samplerate')}
											data-testid="samplerate-select"
											value={value('sampleRate', view().sampleRate)}
											onChoose={choose('sampleRate')}
										>
											<Show when={mixed('sampleRate')}>
												<option value="" disabled>
													Mixed
												</option>
											</Show>
											<For each={view().sampleRateOptions} keyed={(option) => option.value}>
												{(option) => (
													<option value={option().value} disabled={option().disabled}>
														{option().label}
													</option>
												)}
											</For>
										</Select>
										<Show when={view().sampleRateHint}>
											<p
												id={id('output-samplerate-effective')}
												class="field-hint"
												data-testid="auto-samplerate-hint"
											>
												{view().sampleRateHint}
											</p>
										</Show>
									</div>
								</div>

								<div class="encoder-field-row">
									<label for={id('output-channels')}>Channels</label>
									<div class="encoder-field-stack">
										<Select
											id={id('output-channels')}
											data-testid="channels-select"
											value={value('channels', view().channels)}
											onChoose={choose('channels')}
										>
											<Show when={mixed('channels')}>
												<option value="" disabled>
													Mixed
												</option>
											</Show>
											<For each={view().channelOptions} keyed={(option) => option.value}>
												{(option) => <option value={option().value}>{option().label}</option>}
											</For>
										</Select>
										<Show when={view().channelsHint}>
											<p
												id={id('output-channels-effective')}
												class="field-hint"
												data-testid="auto-channels-hint"
											>
												{view().channelsHint}
											</p>
										</Show>
									</div>
								</div>
							</Show>
						</div>
					</details>
				</Show>
			</div>
		</div>
	);
}
