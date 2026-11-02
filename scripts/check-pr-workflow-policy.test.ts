import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { checkWorkflow } from './check-pr-workflow-policy';

const repoRoot = path.resolve(import.meta.dirname, '..');

function messages(file: string, text: string): string[] {
	return checkWorkflow(file, text).map((finding) => `${finding.line}: ${finding.message}`);
}

function changesWorkflow(fetchStep: string, fetchDepth = '2'): string {
	const changes = `
  changes:
    if: \${{ github.event.pull_request.draft != true }}
    runs-on: ubuntu-24.04
    timeout-minutes: 5
    steps:
      - uses: actions/checkout@df4cb1c069e1874edd31b4311f1884172cec0e10
        with:
          fetch-depth: ${fetchDepth}
${fetchStep}      - env:
          BASE: \${{ github.event.pull_request.base.sha }}
        run: bun scripts/verification.ts plan "$BASE"
`;
	return guarded.replace('jobs:\n', `jobs:\n${changes}`);
}

const guarded = `
on:
  pull_request:
    types: [opened, synchronize, reopened, ready_for_review, converted_to_draft, closed]
concurrency:
  group: \${{ github.workflow }}-\${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true
jobs:
  frontend:
    if: \${{ github.event.pull_request.draft != true }}
    runs-on: ubuntu-24.04
    timeout-minutes: 5
`;

describe('check-pr-workflow-policy', () => {
	it('fails the draft leak in the vendored pre-guard workflow', () => {
		const text = readFileSync(
			path.join(repoRoot, 'scripts/fixtures/ci-64ec2240-parent.yml'),
			'utf8',
		);
		const found = messages('.github/workflows/ci.yml', text);
		expect(found.length).toBeGreaterThan(0);
		expect(found.join('\n')).toMatch(/job frontend runs on a draft pull request/);
	});

	it('rejects a scalar pull_request trigger', () => {
		const found = messages(
			'ci.yml',
			'on: pull_request\njobs:\n  frontend:\n    runs-on: ubuntu-24.04\n    timeout-minutes: 5\n',
		);
		expect(found.join('\n')).toMatch(/pull_request types are missing/);
		expect(found.join('\n')).toMatch(/job frontend runs on a draft pull request/);
	});

	it('rejects an array pull_request trigger', () => {
		const found = messages(
			'ci.yml',
			'on: [pull_request]\njobs:\n  frontend:\n    runs-on: ubuntu-24.04\n    timeout-minutes: 5\n',
		);
		expect(found.join('\n')).toMatch(/pull_request types are missing/);
		expect(found.join('\n')).toMatch(/job frontend runs on a draft pull request/);
	});

	it('rejects a draft guard ored with true', () => {
		const found = messages(
			'ci.yml',
			guarded.replace(
				'github.event.pull_request.draft != true',
				'github.event.pull_request.draft != true || true',
			),
		);
		expect(found.join('\n')).toMatch(/bypasses its draft guard/);
	});

	it('rejects a truthy expression in front of the draft guard', () => {
		for (const prefix of ['(true) || ', 'always() || ', '1 == 1 || ']) {
			const found = messages(
				'ci.yml',
				guarded.replace(
					'github.event.pull_request.draft != true',
					`${prefix}github.event.pull_request.draft != true`,
				),
			);
			expect(found.join('\n')).toMatch(/bypasses its draft guard/);
		}
	});

	it('accepts a draft guard anded with a parenthesized or, in either order', () => {
		const clause = "(github.event.pull_request.merged == true || github.event.action != 'closed')";
		const guard = 'github.event.pull_request.draft != true';
		for (const iff of [`${guard} && ${clause}`, `${clause} && ${guard}`]) {
			const found = messages('ci.yml', guarded.replace(guard, iff));
			expect(found.join('\n')).not.toMatch(/bypasses its draft guard|runs on a draft/);
		}
	});

	it('rejects a scalar pull_request_target trigger', () => {
		const found = messages(
			'ci.yml',
			'on: pull_request_target\njobs:\n  frontend:\n    runs-on: ubuntu-24.04\n    timeout-minutes: 5\n',
		);
		expect(found.join('\n')).toMatch(/pull_request_target types are missing/);
		expect(found.join('\n')).toMatch(/job frontend runs on a draft pull request/);
	});

	it('requires closed when the workflow reads that event', () => {
		const dropped = guarded.replace(', closed', '');
		expect(messages('ci.yml', dropped)).toEqual([]);
		const found = messages(
			'ci.yml',
			dropped.replace(
				'github.event.pull_request.draft != true',
				"github.event.pull_request.draft != true && github.event.action == 'closed'",
			),
		);
		expect(found.join('\n')).toMatch(/types are missing closed/);
	});

	it('rejects an unpinned runner, directly or inside a matrix', () => {
		for (const label of ['ubuntu-latest', 'ubuntu-22.04', 'macos-latest', 'macos-15']) {
			const direct = guarded.replace('runs-on: ubuntu-24.04', `runs-on: ${label}`);
			const matrix = guarded.replace(
				'runs-on: ubuntu-24.04',
				`runs-on: \${{ matrix.os }}\n    strategy:\n      matrix:\n        include:\n          - os: ${label}`,
			);
			for (const text of [direct, matrix]) {
				expect(messages('ci.yml', text).join('\n')).toMatch(
					new RegExp(
						`job frontend runs on ${label}.*Fix: pin runs-on to (ubuntu-24\\.04|macos-26)`,
					),
				);
			}
		}
		const pinned = guarded.replace('runs-on: ubuntu-24.04', 'runs-on: macos-26');
		expect(messages('ci.yml', pinned)).toEqual([]);
	});

	it('rejects a draft guard that is not a top-level && conjunct', () => {
		for (const iff of [
			'(github.event.pull_request.draft != true || true)',
			'or(github.event.pull_request.draft != true, true)',
			"github.event.pull_request.draft != true || github.event.action == 'closed'",
		]) {
			const text = guarded.replace('github.event.pull_request.draft != true', iff);
			expect(messages('ci.yml', text).join('\n')).toMatch(
				/job frontend (runs on a draft|bypasses)/,
			);
		}
		const conjunct = guarded.replace(
			'github.event.pull_request.draft != true',
			"(github.event.pull_request.draft != true) && github.event.action != 'closed'",
		);
		expect(messages('ci.yml', conjunct)).toEqual([]);
	});

	it('rejects a branches filter on pull_request', () => {
		const text = guarded.replace(
			'    types: [opened,',
			'    branches: [main]\n    types: [opened,',
		);
		expect(messages('ci.yml', text).join('\n')).toMatch(
			/pull_request branches filter skips stacked/,
		);
	});

	it('accepts push only to main', () => {
		const withPush = (branches: string) =>
			guarded.replace('concurrency:', `  push:\n    branches: ${branches}\nconcurrency:`);
		expect(messages('ci.yml', withPush('[main]'))).toEqual([]);
		for (const branches of ['[main, develop]', "['**']"]) {
			expect(messages('ci.yml', withPush(branches)).join('\n')).toMatch(
				/push runs only to seed main caches/,
			);
		}
	});

	it('applies the push rule to the shorthand trigger list', () => {
		const found = messages(
			'ci.yml',
			'on: [push, pull_request]\njobs:\n  frontend:\n    runs-on: ubuntu-24.04\n    timeout-minutes: 5\n',
		);
		expect(found.join('\n')).toMatch(/push runs only to seed main caches/);
	});

	it('rejects a shallow diff of a base held in job env', () => {
		const text = changesWorkflow('')
			.replace(
				`      - env:\n          BASE: \${{ github.event.pull_request.base.sha }}\n        run: bun`,
				`      - run: bun`,
			)
			.replace(
				'    timeout-minutes: 5\n    steps:',
				`    timeout-minutes: 5\n    env:\n      BASE: \${{ github.event.pull_request.base.sha }}\n    steps:`,
			);
		expect(messages('ci.yml', text).join('\n')).toMatch(/diffs the pull request base/);
	});

	it('rejects a shallow diff of the pull request base', () => {
		const found = messages('ci.yml', changesWorkflow(''));
		expect(found.join('\n')).toMatch(/diffs the pull request base/);
	});

	it('accepts a fetch of the base sha or full history', () => {
		const fetchStep = `
      - env:
          BASE: \${{ github.event.pull_request.base.sha }}
        run: git fetch --no-tags --depth=1 origin "$BASE"
`;
		for (const text of [changesWorkflow(fetchStep), changesWorkflow('', '0')]) {
			expect(messages('ci.yml', text).join('\n')).not.toMatch(
				/did not parse|diffs the pull request base/,
			);
		}
	});

	it('runs an agent review only when a pull request opens ready, reopens, or leaves draft', () => {
		const review = (types: string) => `
on:
  pull_request:
    types: [${types}]
concurrency:
  group: \${{ github.workflow }}-\${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true
jobs:
  review:
    if: \${{ github.event.pull_request.draft != true }}
    runs-on: ubuntu-24.04
    timeout-minutes: 30
    steps:
      - uses: anthropics/claude-code-action@cab360f6565aa35a51d6ce9e43f1f4287c0a32ea
`;
		expect(messages('review.yml', review('opened, ready_for_review, reopened'))).toEqual([]);
		expect(
			messages('review.yml', review('opened, ready_for_review, reopened, synchronize')).join('\n'),
		).toMatch(/agent review runs only when a PR opens ready, reopens, or leaves draft/);
	});

	it('accepts the workflows in this checkout', () => {
		const files = readdirSync(path.join(repoRoot, '.github/workflows')).filter((name) =>
			name.endsWith('.yml'),
		);
		const found = files.flatMap((name) => {
			const file = path.join('.github/workflows', name);
			return messages(file, readFileSync(path.join(repoRoot, file), 'utf8'));
		});
		expect(found).toEqual([]);
	});
});
