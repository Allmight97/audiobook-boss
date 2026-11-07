import type { JSX } from '@solidjs/web';
import { useAppRuntime } from '../../app/runtime';
import './jobControls.css';

export function GroupTitlesButton(): JSX.Element {
	const input = useAppRuntime().input;
	return (
		<button
			type="button"
			class="group-titles-button"
			disabled={!input.view().canGroup}
			title="Select two or more titles to group their files into one audiobook."
			onClick={() => input.groupSelected()}
		>
			Group as one title
		</button>
	);
}
