#!/usr/bin/env bun

import { readFile } from 'node:fs/promises';

import { readExecutionSpecFile, type DeliveryMode, type HumanReviewMode } from './common';
import { createIssue } from './github';
import { runIssue } from './runner';
import { renderIdeaIssueBody, renderReadyIssueBody } from './template';

type RunCommand = { name: 'run'; issue: number };
type CreateCommand = {
	name: 'create';
	mode: 'ready' | 'idea';
	title: string;
	specPath?: string;
	bodyFile?: string;
	deliveryMode: DeliveryMode;
	humanReview: HumanReviewMode;
	printBody: boolean;
	labels: string[];
};

type Command = RunCommand | CreateCommand;

function takeFlag(args: string[], flag: string): string | undefined {
	const index = args.indexOf(flag);
	if (index < 0) return undefined;
	const value = args[index + 1];
	if (!value) {
		throw new Error(`Missing value for ${flag}.`);
	}
	args.splice(index, 2);
	return value;
}

function takeRepeatedFlag(args: string[], flag: string): string[] {
	const values: string[] = [];
	for (;;) {
		const index = args.indexOf(flag);
		if (index < 0) return values;
		const value = args[index + 1];
		if (!value) {
			throw new Error(`Missing value for ${flag}.`);
		}
		values.push(value);
		args.splice(index, 2);
	}
}

function parseCommand(argv: string[]): Command {
	const [command, ...rest] = argv;
	switch (command) {
		case 'run': {
			const issue = Number(takeFlag(rest, '--issue') ?? rest[0]);
			if (!Number.isFinite(issue)) {
				throw new Error('`issue:run` requires --issue <number>.');
			}
			return { name: 'run', issue };
		}
		case 'create': {
			const mode = (takeFlag(rest, '--mode') ?? 'ready') as 'ready' | 'idea';
			const title = takeFlag(rest, '--title');
			if (!title) {
				throw new Error('`issue:create` requires --title <title>.');
			}
			const specPath = takeFlag(rest, '--spec');
			const bodyFile = takeFlag(rest, '--body-file');
			const deliveryMode = (takeFlag(rest, '--delivery-mode') ?? 'pr') as DeliveryMode;
			const humanReview = (takeFlag(rest, '--human-review') ?? 'none') as HumanReviewMode;
			const labels = takeRepeatedFlag(rest, '--label');
			const printBody = rest.includes('--print-body');
			const filtered = rest.filter((entry) => entry !== '--print-body');
			if (filtered.length > 0) {
				throw new Error(`Unexpected arguments: ${filtered.join(' ')}`);
			}
			return {
				name: 'create',
				mode,
				title,
				specPath,
				bodyFile,
				deliveryMode,
				humanReview,
				printBody,
				labels,
			};
		}
		default:
			throw new Error(`Unknown issue command: ${command ?? '(missing command)'}`);
	}
}

async function createIssueCommand(command: CreateCommand): Promise<void> {
	const repoRoot = process.cwd();
	let body: string;

	if (command.mode === 'ready') {
		if (!command.specPath) {
			throw new Error('`issue:create --mode ready` requires --spec <path>.');
		}
		body = renderReadyIssueBody(await readExecutionSpecFile(command.specPath), {
			deliveryMode: command.deliveryMode,
			humanReview: command.humanReview,
		});
	} else {
		const raw = command.bodyFile ? await readFile(command.bodyFile, 'utf8') : '';
		body = renderIdeaIssueBody(raw);
	}

	if (command.printBody) {
		console.log(body);
		return;
	}

	const created = await createIssue(repoRoot, {
		title: command.title,
		body,
		labels: command.labels,
	});
	console.log(JSON.stringify(created, null, 2));
}

async function main(): Promise<void> {
	const command = parseCommand(process.argv.slice(2));
	if (command.name === 'run') {
		console.log(JSON.stringify(await runIssue(command.issue), null, 2));
		return;
	}
	await createIssueCommand(command);
}

main().catch((error) => {
	console.error(`[issue] ${error instanceof Error ? error.message : String(error)}`);
	process.exit(1);
});
