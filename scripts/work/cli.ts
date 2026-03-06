#!/usr/bin/env bun

import { finishTask, gcRunnerState, runNextTask, runTaskById } from './runner';

type Command =
	| { name: 'next' }
	| { name: 'run'; task: string }
	| { name: 'finish'; task: string; disposition: 'merged' | 'abandoned' }
	| { name: 'gc' };

function parseCommand(argv: string[]): Command {
	const [command, ...rest] = argv;
	switch (command) {
		case 'next':
			return { name: 'next' };
		case 'run': {
			const taskFlagIndex = rest.indexOf('--task');
			const task = taskFlagIndex >= 0 ? rest[taskFlagIndex + 1] : rest[0];
			if (!task) {
				throw new Error('Missing task id or task file path for `work:run`.');
			}
			return { name: 'run', task };
		}
		case 'finish': {
			const taskFlagIndex = rest.indexOf('--task');
			const task = taskFlagIndex >= 0 ? rest[taskFlagIndex + 1] : undefined;
			if (!task) {
				throw new Error('Missing --task <id> for `work:finish`.');
			}
			if (rest.includes('--merged')) {
				return { name: 'finish', task, disposition: 'merged' };
			}
			if (rest.includes('--abandoned')) {
				return { name: 'finish', task, disposition: 'abandoned' };
			}
			throw new Error('Finish requires either --merged or --abandoned.');
		}
		case 'gc':
			return { name: 'gc' };
		default:
			throw new Error(`Unknown work command: ${command ?? '(missing command)'}`);
	}
}

async function main(): Promise<void> {
	const command = parseCommand(process.argv.slice(2));
	switch (command.name) {
		case 'next':
			console.log(JSON.stringify(await runNextTask(), null, 2));
			return;
		case 'run':
			console.log(JSON.stringify(await runTaskById(command.task), null, 2));
			return;
		case 'finish':
			console.log(JSON.stringify(await finishTask(command.task, command.disposition), null, 2));
			return;
		case 'gc':
			console.log(JSON.stringify(await gcRunnerState(), null, 2));
			return;
	}
}

main().catch((error) => {
	console.error(`[work] ${error instanceof Error ? error.message : String(error)}`);
	process.exit(1);
});
