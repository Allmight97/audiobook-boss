// oxlint-disable-next-line eslint/no-restricted-imports -- the recorded workflows replay the raw IPC wire.
import { invoke } from '@tauri-apps/api/core';
import { expect, vi } from 'vitest';
import type { TauriCommandName } from '../tauriCommandNames';
import sessionFresh from '../../lib/generated/fixtures/session-fresh.json';
import sessionScenarios from '../../lib/generated/fixtures/session-scenarios.json';
import settingsFresh from '../../lib/generated/fixtures/settings-fresh.json';
import settingsScenarios from '../../lib/generated/fixtures/settings-scenarios.json';

type Scenario = { name: string; start?: unknown; steps: ReadonlyArray<Record<string, unknown>> };

type Replay = {
	/** Releases the reply held with `hold`. */
	release: () => void;
	/** Fails when an intent differed from the recording or a recorded one never came. */
	finish: () => void;
};

/**
 * Answers the commands of one recorded scenario at the `invoke` boundary, so
 * the real client, link and owners run. It holds no state and no
 * rule: each dispatch must carry the next recorded intent and gets the
 * recorded reply unchanged.
 *
 * `through` stops the replay after that many intents. `hold` delays the reply
 * to that numbered intent until `release`.
 */
export function replayInvoke(
	engine: 'session' | 'settings',
	name: string,
	options: { readonly through?: number; readonly hold?: number } = {},
): Replay {
	const scenarios: readonly Scenario[] =
		engine === 'session' ? sessionScenarios : settingsScenarios;
	const scenario = scenarios.find((candidate) => candidate.name === name);
	if (!scenario) throw new Error(`no recorded scenario named ${name}`);
	const steps = scenario.steps.slice(0, options.through ?? scenario.steps.length);
	const wanted: TauriCommandName = engine === 'session' ? 'session_dispatch' : 'settings_dispatch';
	const attach: TauriCommandName = 'attach_frontend';
	const mismatches: string[] = [];
	const fallback = vi.mocked(invoke).getMockImplementation();
	let delivered = 0;
	let release: () => void = () => undefined;
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});

	vi.mocked(invoke).mockImplementation(async (command, args) => {
		if (command === attach) {
			return {
				client: 1,
				session: sessionFresh,
				settings: scenario.start ?? settingsFresh,
				workOperations: { revision: 0, order: [], operations: [] },
			};
		}
		if (command !== wanted) return fallback?.(command, args);
		const sent = args as { sequence: number; intent: unknown };
		const step = steps[delivered];
		const number = delivered + 1;
		delivered += 1;
		if (!step || sent.sequence !== number - 1) {
			mismatches.push(`unexpected ${wanted} number ${sent.sequence}`);
			throw new Error(`unexpected ${wanted} ${JSON.stringify(sent)}`);
		}
		try {
			expect(sent.intent).toEqual(step.intent);
		} catch (error) {
			mismatches.push(`intent ${number} differs from the recording`);
			throw error;
		}
		if (options.hold === number) await held;
		return engine === 'session'
			? { outcome: step.outcome, update: step.update }
			: { outcome: step.outcome, snapshot: step.snapshot };
	});

	return {
		release,
		finish() {
			expect(mismatches).toEqual([]);
			expect(steps.length - delivered).toBe(0);
		},
	};
}
