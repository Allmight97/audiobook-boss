#!/usr/bin/env bun

import {
	captureScreenshot,
	closeHarnessSession,
	getDomSummary,
	reportText,
	runUiReviewChecklist,
	seedScenario,
	startHarnessSession,
	type HarnessViewportPreset,
} from './api';
import type { HarnessScenarioId } from '../../src/harness/scenarios';

type Command =
	| {
			name: 'start';
			scenario?: HarnessScenarioId;
			route?: string;
			viewport?: HarnessViewportPreset;
			headed?: boolean;
	  }
	| { name: 'seed'; scenario: HarnessScenarioId }
	| { name: 'screenshot'; label: string }
	| { name: 'dom' }
	| { name: 'report'; message: string }
	| { name: 'review'; viewport?: HarnessViewportPreset; scenario?: HarnessScenarioId }
	| { name: 'close' };

function parseViewport(value: string | undefined): HarnessViewportPreset | undefined {
	if (!value) return undefined;
	if (value === 'desktop' || value === 'mobile') {
		return value;
	}
	throw new Error(`Unknown viewport preset: ${value}`);
}

function parseCommand(argv: string[]): Command {
	if (argv.length === 0) {
		return { name: 'start' };
	}

	const [command, ...rest] = argv;
	switch (command) {
		case 'start': {
			const parsed: Command = { name: 'start' };
			for (let index = 0; index < rest.length; index += 1) {
				const arg = rest[index];
				const next = rest[index + 1];
				if (arg === '--scenario') {
					if (!next) throw new Error('Missing value for --scenario');
					parsed.scenario = next as HarnessScenarioId;
					index += 1;
					continue;
				}
				if (arg === '--route') {
					if (!next) throw new Error('Missing value for --route');
					parsed.route = next;
					index += 1;
					continue;
				}
				if (arg === '--viewport') {
					parsed.viewport = parseViewport(next);
					index += 1;
					continue;
				}
				if (arg === '--headed') {
					parsed.headed = true;
					continue;
				}
				throw new Error(`Unknown argument for start: ${arg}`);
			}
			return parsed;
		}
		case 'seed':
			if (!rest[0]) throw new Error('Missing scenario id for seed.');
			return { name: 'seed', scenario: rest[0] as HarnessScenarioId };
		case 'screenshot':
			if (!rest[0]) throw new Error('Missing screenshot label.');
			return { name: 'screenshot', label: rest[0] };
		case 'dom':
			return { name: 'dom' };
		case 'report':
			if (rest.length === 0) throw new Error('Missing report message.');
			return { name: 'report', message: rest.join(' ') };
		case 'review': {
			const parsed: Command = { name: 'review' };
			for (let index = 0; index < rest.length; index += 1) {
				const arg = rest[index];
				const next = rest[index + 1];
				if (arg === '--viewport') {
					parsed.viewport = parseViewport(next);
					index += 1;
					continue;
				}
				if (arg === '--scenario') {
					if (!next) throw new Error('Missing value for --scenario');
					parsed.scenario = next as HarnessScenarioId;
					index += 1;
					continue;
				}
				throw new Error(`Unknown argument for review: ${arg}`);
			}
			return parsed;
		}
		case 'close':
			return { name: 'close' };
		default:
			throw new Error(`Unknown harness:agent command: ${command}`);
	}
}

async function main(): Promise<void> {
	const command = parseCommand(process.argv.slice(2));
	switch (command.name) {
		case 'start':
			console.log(
				JSON.stringify(
					await startHarnessSession({
						scenario: command.scenario,
						route: command.route,
						viewport: command.viewport,
						headed: command.headed,
					}),
					null,
					2,
				),
			);
			return;
		case 'seed':
			console.log(JSON.stringify(await seedScenario(command.scenario), null, 2));
			return;
		case 'screenshot':
			console.log(JSON.stringify(await captureScreenshot(command.label), null, 2));
			return;
		case 'dom':
			console.log(JSON.stringify(await getDomSummary(), null, 2));
			return;
		case 'report':
			console.log(JSON.stringify(await reportText(command.message), null, 2));
			return;
		case 'review':
			console.log(
				JSON.stringify(
					await runUiReviewChecklist({
						viewport: command.viewport,
						scenario: command.scenario,
					}),
					null,
					2,
				),
			);
			return;
		case 'close':
			await closeHarnessSession();
			console.log(JSON.stringify({ closed: true }, null, 2));
	}
}

main().catch((error) => {
	console.error(`[harness:agent] ${error instanceof Error ? error.message : String(error)}`);
	process.exit(1);
});
