#!/usr/bin/env bun
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { buildPlan, ProofUsageError } from './catalog';
import { createArtifactWriter, eventTimestamp } from './events';
import { runStep } from './executor';
import { formatCommand } from './format';
import type { ProofPlan, ProofStep, ProofStepResult, ProofSummary } from './types';

const repoRoot = path.resolve(import.meta.dir, '..', '..');

function usage(): string {
	return `Usage:
  bun scripts/proof/runner.ts [review]
  bun scripts/proof/runner.ts review [quick|rust|runtime|frontend]
  bun scripts/proof/runner.ts focus rust lib <filter>
  bun scripts/proof/runner.ts focus rust integration <test-target> [filter]
  bun scripts/proof/runner.ts focus rust contract
  bun scripts/proof/runner.ts focus rust private
  bun scripts/proof/runner.ts focus rust media
  bun scripts/proof/runner.ts focus rust media-manual [all|xhe-aac|native-fastpath]
  bun scripts/proof/runner.ts focus frontend
  bun scripts/proof/runner.ts focus runtime
  bun scripts/proof/runner.ts release [package]
  bun scripts/proof/runner.ts diagnose timing [cargo build args...]
  bun scripts/proof/runner.ts diagnose coverage [rust|ts|all]
  bun scripts/proof/runner.ts diagnose deps

Examples:
  bun scripts/proof/runner.ts review
  bun scripts/proof/runner.ts review quick
  bun scripts/proof/runner.ts focus rust contract
  bun scripts/proof/runner.ts focus rust lib metadata_intent_validation_contract
  bun scripts/proof/runner.ts focus rust integration integration_metadata_tests reads_track
  ABB_XHE_AAC_FIXTURE=/path/to/book.m4b bun scripts/proof/runner.ts focus rust media-manual xhe-aac
`;
}

function parseArgs(args: string[]): string[] {
	if (args[0] === '--route') {
		throw new ProofUsageError('--route is no longer supported. Use the proof category directly.');
	}

	return args;
}

function commandVector(step: ProofStep): string[] {
	return [step.command, ...step.args];
}

function stepStartedEvent(step: ProofStep) {
	const requiredEnv = step.requiredEnv?.length ? step.requiredEnv : undefined;
	return {
		command: commandVector(step),
		kind: 'step_started' as const,
		...(requiredEnv ? { requiredEnv } : {}),
		stepId: step.id,
		timestamp: eventTimestamp(),
	};
}

function printPlanStart(plan: ProofPlan, artifactDir: string): void {
	console.log(`[proof:${plan.id}] ${plan.label}`);
	console.log(`[proof:${plan.id}] ${plan.purpose}`);
	console.log(`[proof:${plan.id}] artifacts: ${artifactDir}`);
}

function printStepStart(plan: ProofPlan, step: ProofStep): void {
	console.log(`[proof:${plan.id}] step ${step.id}: ${formatCommand(step)}`);
}

function printStepResult(plan: ProofPlan, result: ProofStepResult): void {
	const seconds = (result.durationMs / 1000).toFixed(2);
	console.log(
		`[proof:${plan.id}] ${result.status}: ${result.id} (${seconds}s) -> ${result.logPath}`,
	);
}

function printFailureExcerpt(plan: ProofPlan, result: ProofStepResult): void {
	const excerpt = readLogTail(result.logPath, 50);
	if (!excerpt) {
		return;
	}

	console.error(`[proof:${plan.id}] failure excerpt from ${result.id}:`);
	console.error(excerpt);
}

function readLogTail(logPath: string, maxLines: number): string {
	try {
		return readFileSync(logPath, 'utf8')
			.split(/\r?\n/)
			.filter((line) => line.trim().length > 0)
			.slice(-maxLines)
			.join('\n');
	} catch {
		return '';
	}
}

function makeSummary(
	artifactDir: string,
	durationMs: number,
	plan: ProofPlan,
	steps: ProofStepResult[],
): ProofSummary {
	const failedStep = steps.find((step) => step.status === 'failed');
	return {
		artifactDir,
		durationMs,
		failedStepId: failedStep?.id,
		plan: {
			classification: plan.classification,
			id: plan.id,
			label: plan.label,
			purpose: plan.purpose,
		},
		status: failedStep ? 'failed' : 'passed',
		steps,
	};
}

async function runPlan(plan: ProofPlan): Promise<number> {
	const artifacts = createArtifactWriter(repoRoot, plan.id);
	const startedAt = Date.now();
	const stepResults: ProofStepResult[] = [];

	artifacts.record({
		artifactDir: artifacts.artifactDir,
		kind: 'run_started',
		planId: plan.id,
		timestamp: eventTimestamp(),
	});
	printPlanStart(plan, artifacts.artifactDir);

	for (const step of plan.steps) {
		const logPath = path.join(artifacts.logsDir, `${step.id}.log`);
		printStepStart(plan, step);
		artifacts.record(stepStartedEvent(step));

		const result = await runStep(step, { logPath, repoRoot });
		stepResults.push(result);
		printStepResult(plan, result);
		artifacts.record({
			durationMs: result.durationMs,
			exitCode: result.exitCode,
			kind: 'step_finished',
			logPath,
			status: result.status,
			stepId: result.id,
			timestamp: eventTimestamp(),
		});

		if (result.status === 'failed') {
			printFailureExcerpt(plan, result);
			break;
		}
	}

	const summary = makeSummary(artifacts.artifactDir, Date.now() - startedAt, plan, stepResults);
	const summaryPaths = artifacts.writeSummary(summary);
	artifacts.record({ kind: 'run_finished', status: summary.status, timestamp: eventTimestamp() });

	if (summary.status === 'failed') {
		artifacts.record({
			kind: 'next_action_hint',
			message: `Inspect ${summary.failedStepId ?? 'failed step'} log before rerunning broader proof.`,
			timestamp: eventTimestamp(),
		});
		console.error(`[proof:${plan.id}] failed. summary: ${summaryPaths.markdownPath}`);
		return 1;
	}

	console.log(`[proof:${plan.id}] passed. summary: ${summaryPaths.markdownPath}`);
	return 0;
}

async function main(): Promise<number> {
	const args = process.argv.slice(2);
	if (args.includes('--help') || args.includes('-h')) {
		console.log(usage());
		return 0;
	}

	try {
		return await runPlan(buildPlan(parseArgs(args)));
	} catch (error) {
		if (error instanceof ProofUsageError) {
			console.error(`[proof] ${error.message}`);
			console.error(usage());
			return 2;
		}
		throw error;
	}
}

process.exitCode = await main();
