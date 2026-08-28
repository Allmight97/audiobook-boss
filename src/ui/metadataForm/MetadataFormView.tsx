import { For, type JSX } from 'solid-js';
import { runLookupActionAtom } from '../../app/metadataLookup';
import {
	METADATA_FIELD_DEFINITIONS,
	metadataViewAtom,
	saveMetadataAtom,
	setMetadataFieldActionAtom,
	setMetadataFieldValueAtom,
} from '../../app/metadataSession';
import { useAtomSet, useAtomValue } from '../../app/runtime/solid';
import './metadataForm.css';

const FIELD_SPAN_CLASS = {
	1: 'col-span-1',
	2: 'col-span-2',
	3: 'col-span-3',
	4: 'col-span-4',
} as const;

export function MetadataFormView(): JSX.Element {
	const view = useAtomValue(() => metadataViewAtom);
	const setFieldValue = useAtomSet(() => setMetadataFieldValueAtom);
	const setFieldAction = useAtomSet(() => setMetadataFieldActionAtom);
	const saveMetadata = useAtomSet(() => saveMetadataAtom);
	const openLookup = useAtomSet(() => runLookupActionAtom);
	const form = () => view().form;

	return (
		<>
			<div
				class="grid grid-cols-4 gap-x-3 gap-y-1.5"
				onInput={(event) => {
					const target = event.target;
					if (
						(target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) &&
						target.id.startsWith('meta-')
					) {
						setFieldValue({ inputId: target.id, value: target.value });
					}
				}}
				onChange={(event) => {
					const target = event.target;
					if (
						target instanceof HTMLSelectElement &&
						target.classList.contains('meta-apply-select')
					) {
						setFieldAction({
							actionId: target.id,
							action: target.value === 'blank' ? 'blank' : 'keep',
						});
					}
				}}
			>
				<For each={[...METADATA_FIELD_DEFINITIONS]}>
					{(field) => {
						const fieldState = () => form().fields[field.inputId];
						return (
							<div class={FIELD_SPAN_CLASS[field.span]}>
								<div class="meta-field-header">
									<label for={field.inputId}>{field.label}</label>
									{form().mode === 'multi' && (
										<select
											id={field.actionId}
											class="meta-apply-select"
											data-testid={field.actionId}
											value={fieldState().action}
										>
											<option value="keep">Keep</option>
											<option value="blank">Blank</option>
										</select>
									)}
								</div>
								{field.kind === 'textarea' ? (
									<textarea
										id={field.inputId}
										rows={2}
										value={fieldState().value}
										placeholder={fieldState().mixed ? 'Mixed values' : fieldState().placeholder}
										data-dirty={fieldState().dirty ? 'true' : undefined}
										data-mixed={fieldState().mixed ? 'true' : undefined}
										classList={{ 'dirty-field': fieldState().dirty }}
									/>
								) : (
									<input
										type="text"
										id={field.inputId}
										value={fieldState().value}
										placeholder={fieldState().mixed ? 'Mixed values' : fieldState().placeholder}
										data-dirty={fieldState().dirty ? 'true' : undefined}
										data-mixed={fieldState().mixed ? 'true' : undefined}
										classList={{ 'dirty-field': fieldState().dirty }}
									/>
								)}
								{field.inputId === 'meta-series-part' && (
									<div
										id="meta-series-part-warning"
										class="text-xs warning-text"
										hidden={!form().seriesPartWarning.visible}
									>
										{form().seriesPartWarning.message}
									</div>
								)}
								{field.inputId === 'meta-subseries-part' && (
									<div
										id="meta-subseries-part-warning"
										class="text-xs warning-text"
										hidden={!form().subseriesPartWarning.visible}
									>
										{form().subseriesPartWarning.message}
									</div>
								)}
							</div>
						);
					}}
				</For>
			</div>
			<div class="metadata-apply-row">
				<button
					id="metadata-lookup-btn"
					class="btn-pill btn-pill-secondary"
					data-testid="metadata-lookup-btn"
					type="button"
					onClick={() => void openLookup({ type: 'open' })}
				>
					Find Metadata
				</button>
				<button
					id="metadata-save-btn"
					class="btn-pill btn-pill-primary"
					data-testid="metadata-save-btn"
					type="button"
					disabled={view().saveInProgress}
					onClick={() => void saveMetadata(undefined)}
				>
					Save All Changes
				</button>
			</div>
		</>
	);
}
