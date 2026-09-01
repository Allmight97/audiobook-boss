import { Show, createSignal, type JSX } from 'solid-js';
import { ProductionRoot } from '../app/runtime/ProductionRoot';
import { ScenarioSwitcher } from './ScenarioSwitcher';
import { MOCK_SCENARIO_IDS, applyScenario, type MockScenarioId } from './runtime';

function scenarioFromSearch(): MockScenarioId {
	const requested = new URLSearchParams(window.location.search).get('scenario');
	return MOCK_SCENARIO_IDS.find((id) => id === requested) ?? 'empty';
}

export function MockRoot(): JSX.Element {
	const initial = scenarioFromSearch();
	applyScenario(initial);
	const [scenario, setScenario] = createSignal<MockScenarioId>(initial);
	const [epoch, setEpoch] = createSignal(1);

	function select(next: MockScenarioId): void {
		applyScenario(next);
		setScenario(next);
		setEpoch((value) => value + 1);
	}

	return (
		<>
			<ScenarioSwitcher current={scenario()} onSelect={select} />
			<Show when={epoch()} keyed>
				<ProductionRoot />
			</Show>
		</>
	);
}
