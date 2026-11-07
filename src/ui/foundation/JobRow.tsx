import { For } from 'solid-js';
import type { JSX } from '@solidjs/web';
import type { ChildJobStatus } from '../../lib/generated/tauri';

export type JobRowAction = {
	readonly id: string;
	readonly label: string;
	readonly title?: string;
	readonly disabled?: boolean;
	readonly onAction: () => void;
};

export type JobRowProps = {
	readonly status: ChildJobStatus;
	readonly label: string;
	readonly labelTitle?: string;
	readonly statusText: string;
	/** Each caller injects its own intents; a row keeps its buttons across a progress tick. */
	readonly actions: ReadonlyArray<JobRowAction>;
	readonly class?: string;
};

export function JobRow(props: JobRowProps): JSX.Element {
	return (
		<div class={['abb-job-row', props.class, `is-${props.status}`]} data-status={props.status}>
			<span class="abb-job-row-label" title={props.labelTitle ?? props.label}>
				{props.label}
			</span>
			<span class="abb-job-row-status">{props.statusText}</span>
			<For each={props.actions} keyed={(action) => action.id}>
				{(action) => (
					<button
						class="abb-job-row-action"
						type="button"
						title={action().title}
						disabled={action().disabled}
						onClick={() => action().onAction()}
					>
						{action().label}
					</button>
				)}
			</For>
		</div>
	);
}
