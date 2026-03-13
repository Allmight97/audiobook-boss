#!/usr/bin/env bun

const retiredMessage = [
	'The markdown-inbox Workloop CLI is retired.',
	'Use `bun run issue:run --issue <number>` for execution-ready GitHub issues.',
	'See `README.md`, `AGENTS.md`, and `WORKFLOW.md` for the active ABB execution contract.',
].join(' ');

console.error(`[work] ${retiredMessage}`);
process.exit(1);
