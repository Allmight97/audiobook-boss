#!/usr/bin/env bun

import { spawnSync } from 'node:child_process';
import { resolve } from 'node:path';

const repoRoot = resolve(import.meta.dir, '..', '..');
const runner = resolve(import.meta.dir, 'run.mjs');

function runPerfMode(mode, runs) {
	const proc = spawnSync(
		'bun',
		[
			runner,
			'--all',
			'--mode',
			mode,
			'--runs',
			String(runs),
			'--compare-baseline',
			'--append-history',
		],
		{
			cwd: repoRoot,
			stdio: 'inherit',
			env: process.env,
		},
	);
	return proc.status ?? 1;
}

const syntheticStatus = runPerfMode('synthetic', 9);
const realStatus = runPerfMode('real', 5);
process.exitCode = syntheticStatus === 0 && realStatus === 0 ? 0 : 1;
