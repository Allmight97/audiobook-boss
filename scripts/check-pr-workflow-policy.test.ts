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
	// Fixture is 64ec2240^:.github/workflows/ci.yml, before drafts were skipped.
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

	it('rejects ubuntu-latest inside a matrix the job runs on', () => {
		const text = guarded.replace(
			'runs-on: ubuntu-24.04',
			`runs-on: \${{ matrix.os }}\n    strategy:\n      matrix:\n        os: [ubuntu-latest]`,
		);
		const found = messages('ci.yml', text);
		expect(found.join('\n')).toMatch(/job frontend uses ubuntu-latest/);
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
