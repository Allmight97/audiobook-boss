import { For } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { METADATA_FIELD_DEFINITIONS } from '../../app/metadataSession';
import { useAppRuntime } from '../../app/runtime';
import { Button } from '../foundation';
import './metadataForm.css';

const FIELD_SPAN_CLASS = {
	1: 'metadata-span-1',
	2: 'metadata-span-2',
	3: 'metadata-span-3',
	4: 'metadata-span-4',
} as const;

export function MetadataFormView(): JSX.Element {
	const metadata = useAppRuntime().metadata;
	const view = metadata.view;
	const setFieldValue = metadata.setFieldValue;
	const setFieldAction = metadata.setFieldAction;
	const saveMetadata = metadata.save;
	const openLookup = useAppRuntime().lookup.run;
	const form = () => view().form;

	return (
		<>
			<div
				class="metadata-form-grid"
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
										class={{ 'dirty-field': fieldState().dirty }}
									/>
								) : (
									<input
										type="text"
										id={field.inputId}
										value={fieldState().value}
										placeholder={fieldState().mixed ? 'Mixed values' : fieldState().placeholder}
										data-dirty={fieldState().dirty ? 'true' : undefined}
										data-mixed={fieldState().mixed ? 'true' : undefined}
										class={{ 'dirty-field': fieldState().dirty }}
									/>
								)}
								{field.inputId === 'meta-series-part' && (
									<div
										id="meta-series-part-warning"
										class="warning-text"
										hidden={!form().seriesPartWarning.visible}
									>
										{form().seriesPartWarning.message}
									</div>
								)}
								{field.inputId === 'meta-subseries-part' && (
									<div
										id="meta-subseries-part-warning"
										class="warning-text"
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
				<Button
					id="metadata-lookup-btn"
					data-testid="metadata-lookup-btn"
					onClick={() => void openLookup({ type: 'open' })}
				>
					Find Metadata
				</Button>
				<Button
					id="metadata-save-btn"
					tone="primary"
					data-testid="metadata-save-btn"
					disabled={view().saveInProgress}
					onClick={() => void saveMetadata()}
				>
					Save All Changes
				</Button>
			</div>
		</>
	);
}
