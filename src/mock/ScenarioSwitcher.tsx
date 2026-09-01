import { For, type JSX } from 'solid-js';
import { Button } from '../ui/foundation';
import { MOCK_SCENARIO_IDS, MOCK_SCENARIO_LABELS, type MockScenarioId } from './runtime';
import './mock.css';

export function ScenarioSwitcher(props: {
	readonly current: MockScenarioId;
	readonly onSelect: (scenario: MockScenarioId) => void;
}): JSX.Element {
	return (
		<aside class="ui-mock-switcher" data-testid="ui-mock-scenario-switcher">
			<p class="ui-mock-switcher-title">Mock runtime</p>
			<p class="ui-mock-switcher-note">No Rust. Fixtures only.</p>
			<fieldset class="ui-mock-switcher-actions">
				<legend class="ui-mock-switcher-legend">Mock scenarios</legend>
				<For each={[...MOCK_SCENARIO_IDS]}>
					{(scenario) => (
						<Button
							tone={props.current === scenario ? 'primary' : 'secondary'}
							data-testid={`ui-mock-scenario-${scenario}`}
							aria-pressed={props.current === scenario}
							onClick={() => props.onSelect(scenario)}
						>
							{MOCK_SCENARIO_LABELS[scenario]}
						</Button>
					)}
				</For>
			</fieldset>
		</aside>
	);
}
