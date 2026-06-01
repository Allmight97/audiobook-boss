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
  bun scripts/proof/runner.ts review [quick|core|rust|runtime|frontend]
  bun scripts/proof/runner.ts focus core <metadata|output-artifact|processing|remote-source>
  bun scripts/proof/runner.ts focus rust lib <filter>
  bun scripts/proof/runner.ts focus rust integration <test-module> [filter]
  bun scripts/proof/runner.ts focus rust contract
  bun scripts/proof/runner.ts focus frontend
  bun scripts/proof/runner.ts focus runtime
  bun scripts/proof/runner.ts release [package]
  bun scripts/proof/runner.ts diagnose timing [cargo build args...]
  bun scripts/proof/runner.ts diagnose coverage [rust|ts|all]
  bun scripts/proof/runner.ts diagnose deps
  bun scripts/proof/runner.ts diagnose rust-target

Requirements:
  cargo-nextest for full Rust proof: cargo install cargo-nextest --locked

Examples:
  bun scripts/proof/runner.ts review
  bun scripts/proof/runner.ts review quick
  bun scripts/proof/runner.ts review core
  bun scripts/proof/runner.ts focus core metadata
  bun scripts/proof/runner.ts focus rust contract
  bun scripts/proof/runner.ts focus rust lib metadata_intent_validation_contract
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

function printPlanStart(plan: ProofPlan): void {
	console.log(`[proof:${plan.id}] ${plan.label}`);
	console.log(`[proof:${plan.id}] ${plan.purpose}`);
	console.log(`[proof:${plan.id}] logs: temp evidence kept only on failure`);
}

function printStepStart(plan: ProofPlan, step: ProofStep): void {
	console.log(`[proof:${plan.id}] step ${step.id}: ${formatCommand(step)}`);
}

function printStepResult(plan: ProofPlan, result: ProofStepResult): void {
	const seconds = (result.durationMs / 1000).toFixed(2);
	const logHint = result.status === 'failed' ? ` -> ${result.logPath}` : '';
	console.log(`[proof:${plan.id}] ${result.status}: ${result.id} (${seconds}s)${logHint}`);
}

function printFailureExcerpt(plan: ProofPlan, result: ProofStepResult): void {
	const excerpt = readLogTail(result.logPath, 50);
	if (!excerpt) {
		return;
	}

	console.error(`[proof:${plan.id}] failure excerpt from ${result.id}:`);
	console.error(excerpt);
}

function printSuccessReport(plan: ProofPlan, result: ProofStepResult): void {
	if (!result.reportOnSuccess) {
		return;
	}

	const report = readLogOutputTail(result.logPath, 80);
	if (!report) {
		return;
	}

	console.log(`[proof:${plan.id}] report from ${result.id}:`);
	console.log(report);
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

function readLogOutputTail(logPath: string, maxLines: number): string {
	try {
		const lines = readFileSync(logPath, 'utf8').split(/\r?\n/);
		const body = lines[0]?.startsWith('$ ') ? lines.slice(2) : lines;
		return body
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

function printTerminalSummary(summary: ProofSummary): void {
	const seconds = (summary.durationMs / 1000).toFixed(2);
	console.log(`[proof:${summary.plan.id}] summary: ${summary.status} (${seconds}s)`);
	for (const step of summary.steps) {
		console.log(`  ${step.status === 'passed' ? 'OK' : 'FAIL'} ${step.id}`);
	}
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
	printPlanStart(plan);

	for (const step of plan.steps) {
		const logPath = path.join(artifacts.logsDir, `${step.id}.log`);
		printStepStart(plan, step);
		artifacts.record(stepStartedEvent(step));

		const result = await runStep(step, { logPath, repoRoot });
		stepResults.push(result);
		printStepResult(plan, result);
		if (result.status === 'passed') {
			printSuccessReport(plan, result);
		}
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
	artifacts.record({ kind: 'run_finished', status: summary.status, timestamp: eventTimestamp() });
	printTerminalSummary(summary);

	if (summary.status === 'failed') {
		artifacts.record({
			kind: 'next_action_hint',
			message: `Inspect ${summary.failedStepId ?? 'failed step'} log before rerunning broader proof.`,
			timestamp: eventTimestamp(),
		});
		const summaryPaths = artifacts.writeSummary(summary);
		console.error(`[proof:${plan.id}] failed artifacts: ${artifacts.artifactDir}`);
		console.error(`[proof:${plan.id}] failed summary: ${summaryPaths.markdownPath}`);
		return 1;
	}

	artifacts.discard();
	console.log(`[proof:${plan.id}] passed. temporary logs discarded.`);
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
