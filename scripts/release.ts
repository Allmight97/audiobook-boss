/**
 * Runs one release lane to the first failure, printing `phase: <name>` lines.
 *
 * Usage: bun scripts/release.ts <x.y.z> <lane>
 *
 * Lanes:
 *   local         bump the version and install /Applications/AudioBook Boss.app
 *   dmg           bump the version and build, verify, and hash the portable DMG
 *   github        dmg, plus audit, commit, push, wait for CI, tag, and publish
 *   github+local  github, plus the local install after the DMG is verified
 *
 * Every lane first checks, without changing anything: macOS, branch `main`,
 * not behind `origin/main`, a tree clean apart from `CHANGELOG.md` and the
 * version files, and a non-empty `## [x.y.z] - <today>` changelog section.
 * The github lanes also need the tag and the GitHub Release to be absent; the
 * install lanes need the installed app to be closed. A rerun after a failure
 * resumes: the bump and commit are no-ops once done.
 */
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { VERSION_SURFACES } from './bump-version';
import { resolveReleaseDmgArtifact } from './resolve-release-dmg';

export const LANES = ['local', 'dmg', 'github', 'github+local'] as const;
export type Lane = (typeof LANES)[number];

const INSTALLED_APP = '/Applications/AudioBook Boss.app';
const BUILT_APP = 'target/release/bundle/macos/AudioBook Boss.app';
const RELEASE_FILES = ['CHANGELOG.md', ...VERSION_SURFACES];
const CI_APPEAR_TIMEOUT_MS = 5 * 60_000;
const CI_FINISH_TIMEOUT_MS = 120 * 60_000;

const USAGE = `Usage: bun scripts/release.ts <x.y.z> <${LANES.join('|')}>`;

export interface Release {
	version: string;
	lane: Lane;
}

export function parseArgs(args: string[]): Release {
	const [version, lane, ...extra] = args;
	if (version === undefined || lane === undefined || extra.length > 0) {
		throw new Error(USAGE);
	}
	if (!/^\d+\.\d+\.\d+$/.test(version)) {
		throw new Error(`The version must be x.y.z, got '${version}'.\n${USAGE}`);
	}
	if (!(LANES as readonly string[]).includes(lane)) {
		throw new Error(`Unknown lane '${lane}'.\n${USAGE}`);
	}
	return { version, lane: lane as Lane };
}

export type Step =
	| 'bump version'
	| 'audit'
	| 'build dmg'
	| 'check bundled licenses'
	| 'install app'
	| 'check installed licenses'
	| 'commit'
	| 'push main'
	| 'wait for ci'
	| 'tag'
	| 'publish release'
	| 'compare asset'
	| 'check remote parity'
	| 'clean';

const DMG_STEPS: Step[] = ['build dmg', 'check bundled licenses'];
const INSTALL_STEPS: Step[] = ['install app', 'check installed licenses'];
const PUBLISH_STEPS: Step[] = [
	'commit',
	'push main',
	'wait for ci',
	'tag',
	'publish release',
	'compare asset',
	'check remote parity',
	'clean',
];

export function stepsFor(lane: Lane): Step[] {
	switch (lane) {
		case 'local':
			return ['bump version', ...INSTALL_STEPS];
		case 'dmg':
			return ['bump version', ...DMG_STEPS];
		case 'github':
			return ['bump version', 'audit', ...DMG_STEPS, ...PUBLISH_STEPS];
		case 'github+local':
			return ['bump version', 'audit', ...DMG_STEPS, ...INSTALL_STEPS, ...PUBLISH_STEPS];
	}
}

const publishes = (lane: Lane) => lane === 'github' || lane === 'github+local';
const installs = (lane: Lane) => lane === 'local' || lane === 'github+local';

function localDate(date: Date): string {
	const pad = (value: number) => String(value).padStart(2, '0');
	return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** The notes under `## [version] - today`, or why they cannot be used. */
export function changelogSection(changelog: string, version: string, today: string): string {
	const lines = changelog.split('\n');
	const heading = `## [${version}]`;
	const start = lines.findIndex((line) => line.startsWith(`${heading} `) || line === heading);
	if (start === -1) {
		throw new Error(
			`CHANGELOG.md has no '${heading} - ${today}' section. Write the user-facing notes first; they become the GitHub Release notes.`,
		);
	}
	if (lines[start] !== `${heading} - ${today}`) {
		throw new Error(`CHANGELOG.md heading '${lines[start]}' must read '${heading} - ${today}'.`);
	}
	const next = lines.findIndex((line, index) => index > start && line.startsWith('## '));
	const body = lines
		.slice(start + 1, next === -1 ? undefined : next)
		.join('\n')
		.trim();
	const entries = body
		.split('\n')
		.filter((line) => line.trim().length > 0 && !line.startsWith('#'));
	if (entries.length === 0) {
		throw new Error(`CHANGELOG.md section '${heading}' has no entries.`);
	}
	return `${body}\n`;
}

/** Paths in `git status --porcelain` output that a release must not carry along. */
export function unexpectedChanges(porcelain: string): string[] {
	return porcelain
		.split('\n')
		.filter((line) => line.length > 3)
		.map((line) => line.slice(3))
		.filter((file) => !RELEASE_FILES.includes(file));
}

export interface CiRun {
	databaseId: number;
	headSha: string;
	event: string;
	status: string;
	conclusion: string;
	url: string;
}

/**
 * The push-triggered `ci` run of `sha`. `gh run list --commit` matches only a
 * full SHA, so a short one would find nothing.
 */
export function ciRunFor(runs: CiRun[], sha: string): CiRun | undefined {
	if (!/^[0-9a-f]{40}$/.test(sha)) {
		throw new Error(`CI runs are found by the full 40-character commit SHA, got '${sha}'.`);
	}
	return runs
		.filter((candidate) => candidate.headSha === sha && candidate.event === 'push')
		.toSorted((left, right) => right.databaseId - left.databaseId)[0];
}

/** Why a finished CI run blocks the tag, or undefined when it passed. */
export function ciFailure(ciRun: CiRun, remoteMain: string): string | undefined {
	if (ciRun.conclusion === 'success') return undefined;
	const where = `CI run ${ciRun.databaseId} for ${ciRun.headSha}`;
	if (ciRun.conclusion === 'cancelled' && remoteMain !== ciRun.headSha) {
		return `${where} was cancelled because a later push replaced it: origin/main is now ${remoteMain}. Nothing was tagged. Pull, check that the newer commits belong in this release, and run the release again; it then waits for the newer commit's run.`;
	}
	if (ciRun.conclusion === 'cancelled') {
		return `${where} was cancelled, and no later push replaced it. Nothing was tagged. Rerun it with 'gh run rerun ${ciRun.databaseId}' and run the release again.`;
	}
	return `${where} ended '${ciRun.conclusion}' (${ciRun.url}). Nothing was tagged. Fix main and run the release again.`;
}

export interface TauriBundle {
	bundle?: { resources?: unknown };
}

/** The bundle's `resources` map: source path (relative to `src-tauri/`) to shipped path. */
export function bundleResources(config: TauriBundle): [string, string][] {
	const resources = config.bundle?.resources;
	if (!resources || typeof resources !== 'object' || Array.isArray(resources)) {
		throw new Error(
			'src-tauri/tauri.conf.json bundle.resources must map each source to its target.',
		);
	}
	return Object.entries(resources as Record<string, string>);
}

/** Each listed resource that the app's `Contents/Resources` lacks or holds changed. */
export function bundleResourceProblems(
	resources: [string, string][],
	srcTauriDir: string,
	resourcesDir: string,
): string[] {
	return resources.flatMap(([source, target]) => {
		const shipped = path.join(resourcesDir, target);
		if (!existsSync(shipped)) return [`${target} is missing (from ${source})`];
		const expected = readFileSync(path.resolve(srcTauriDir, source));
		return readFileSync(shipped).equals(expected) ? [] : [`${target} differs from ${source}`];
	});
}

function run(command: string, args: string[]): void {
	const result = spawnSync(command, args, { stdio: 'inherit' });
	if (result.error) throw result.error;
	if (result.status !== 0) {
		throw new Error(`'${[command, ...args].join(' ')}' exited ${result.status}.`);
	}
}

function capture(command: string, args: string[]) {
	const result = spawnSync(command, args, { encoding: 'utf8' });
	if (result.error) throw result.error;
	return { status: result.status ?? 1, stdout: result.stdout, stderr: result.stderr.trim() };
}

function read(command: string, args: string[]): string {
	const result = capture(command, args);
	if (result.status !== 0) {
		throw new Error(`'${[command, ...args].join(' ')}' exited ${result.status}: ${result.stderr}`);
	}
	return result.stdout.trimEnd();
}

function sha256(file: string): string {
	return createHash('sha256').update(readFileSync(file)).digest('hex');
}

function remoteRef(ref: string): string {
	return read('git', ['ls-remote', 'origin', ref]).split(/\s+/)[0] ?? '';
}

function preconditionProblems(repoRoot: string, release: Release, today: Date): string[] {
	const { version, lane } = release;
	const problems: string[] = [];
	const check = (test: () => string | undefined) => {
		try {
			const problem = test();
			if (problem) problems.push(problem);
		} catch (error) {
			problems.push(error instanceof Error ? error.message : String(error));
		}
	};
	check(() => (process.platform === 'darwin' ? undefined : 'Releases build on macOS only.'));
	check(() => {
		const branch = read('git', ['symbolic-ref', '--quiet', '--short', 'HEAD']);
		return branch === 'main' ? undefined : `Release from main; HEAD is on '${branch}'.`;
	});
	check(() => {
		run('git', ['fetch', '--quiet', 'origin', 'main']);
		const behind = capture('git', ['merge-base', '--is-ancestor', 'origin/main', 'HEAD']);
		return behind.status === 0
			? undefined
			: 'HEAD is behind or apart from origin/main. Pull before releasing.';
	});
	check(() => {
		const stray = unexpectedChanges(
			read('git', ['status', '--porcelain=v1', '--untracked-files=all']),
		);
		return stray.length === 0
			? undefined
			: `The tree has changes a release would not commit: ${stray.join(', ')}.`;
	});
	check(() => {
		changelogSection(
			readFileSync(path.join(repoRoot, 'CHANGELOG.md'), 'utf8'),
			version,
			localDate(today),
		);
		return undefined;
	});
	if (publishes(lane)) {
		const tag = `v${version}`;
		check(() =>
			capture('git', ['rev-parse', '--quiet', '--verify', `refs/tags/${tag}`]).status === 0
				? `Tag ${tag} already exists locally. Resolve it by hand; a release never moves a tag.`
				: undefined,
		);
		check(() =>
			remoteRef(`refs/tags/${tag}`)
				? `Tag ${tag} already exists on origin. Resolve it by hand; a release never moves a tag.`
				: undefined,
		);
		check(() => {
			const view = capture('gh', ['release', 'view', tag, '--json', 'tagName']);
			if (view.status === 0) return `GitHub Release ${tag} already exists.`;
			return /not found/i.test(view.stderr)
				? undefined
				: `Cannot ask GitHub about release ${tag}: ${view.stderr}`;
		});
	}
	if (installs(lane)) {
		check(() => {
			const running = capture('pgrep', ['-f', `${INSTALLED_APP}/Contents/MacOS/`]);
			return running.status === 0
				? `Quit ${INSTALLED_APP} before installing; it is running (pid ${running.stdout.trim().split('\n').join(', ')}).`
				: undefined;
		});
	}
	return problems;
}

interface Context {
	repoRoot: string;
	release: Release;
	tag: string;
	notes: string;
	dmg?: string;
	dmgSha?: string;
	releaseSha?: string;
	releaseUrl?: string;
}

function checkLicenses(context: Context, app: string): void {
	const config = JSON.parse(
		readFileSync(path.join(context.repoRoot, 'src-tauri/tauri.conf.json'), 'utf8'),
	) as TauriBundle;
	const problems = bundleResourceProblems(
		bundleResources(config),
		path.join(context.repoRoot, 'src-tauri'),
		path.join(app, 'Contents/Resources'),
	);
	if (problems.length > 0) {
		throw new Error(`${app} does not ship its license notices:\n- ${problems.join('\n- ')}`);
	}
	console.log(`Licenses in ${app} match tauri.conf.json bundle.resources.`);
}

function waitForCi(sha: string): void {
	const started = Date.now();
	const elapsed = () => `${Math.round((Date.now() - started) / 60_000)} min`;
	const fields = 'databaseId,headSha,event,status,conclusion,url';
	let ciRun: CiRun | undefined;
	while (!ciRun) {
		const runs = JSON.parse(
			read('gh', ['run', 'list', '--commit', sha, '--workflow', 'ci', '--json', fields]),
		) as CiRun[];
		ciRun = ciRunFor(runs, sha);
		if (ciRun) break;
		if (Date.now() - started > CI_APPEAR_TIMEOUT_MS) {
			throw new Error(`No push-triggered ci run appeared for ${sha} within 5 min.`);
		}
		Bun.sleepSync(10_000);
	}
	console.log(`Waiting for CI run ${ciRun.databaseId}: ${ciRun.url}`);
	while (ciRun.status !== 'completed') {
		if (Date.now() - started > CI_FINISH_TIMEOUT_MS) {
			throw new Error(`CI run ${ciRun.databaseId} is still ${ciRun.status} after ${elapsed()}.`);
		}
		Bun.sleepSync(60_000);
		ciRun = JSON.parse(
			read('gh', ['run', 'view', String(ciRun.databaseId), '--json', fields]),
		) as CiRun;
		console.log(`phase: wait for ci (${ciRun.status}, ${elapsed()})`);
	}
	const failure = ciFailure(ciRun, remoteRef('refs/heads/main'));
	if (failure) throw new Error(failure);
}

const STEP_ACTIONS: Record<Step, (context: Context) => void> = {
	'bump version': ({ release }) => run('bun', ['scripts/bump-version.ts', release.version]),
	audit: () => run('bun', ['run', 'audit']),
	'build dmg': (context) => {
		run('bun', ['run', 'app:build:dmg']);
		context.dmg = resolveReleaseDmgArtifact(context.repoRoot, context.release.version);
		run('hdiutil', ['verify', context.dmg]);
		context.dmgSha = sha256(context.dmg);
		console.log(`DMG ${context.dmg}\nSHA-256 ${context.dmgSha}`);
	},
	'check bundled licenses': (context) =>
		checkLicenses(context, path.join(context.repoRoot, BUILT_APP)),
	'install app': () => run('bun', ['run', 'app:install-local']),
	'check installed licenses': (context) => checkLicenses(context, INSTALLED_APP),
	commit: ({ tag }) => {
		run('git', ['add', '--', ...RELEASE_FILES]);
		if (capture('git', ['diff', '--cached', '--quiet']).status === 0) {
			console.log('Nothing to commit; the release is HEAD.');
			return;
		}
		run('git', ['commit', '-m', `rel: release ${tag}`]);
	},
	'push main': (context) => {
		run('git', ['push', 'origin', 'main']);
		context.releaseSha = read('git', ['rev-parse', 'HEAD']);
	},
	'wait for ci': ({ releaseSha }) => waitForCi(releaseSha ?? ''),
	tag: ({ tag, releaseSha }) => {
		run('git', ['tag', tag, releaseSha ?? '']);
		run('git', ['push', 'origin', tag]);
	},
	'publish release': (context) => {
		const notesDir = mkdtempSync(path.join(os.tmpdir(), 'abb-release-notes-'));
		try {
			const notes = path.join(notesDir, 'notes.md');
			writeFileSync(notes, context.notes);
			run('gh', [
				'release',
				'create',
				context.tag,
				context.dmg ?? '',
				'--verify-tag',
				'--title',
				`AudioBook Boss ${context.tag}`,
				'--notes-file',
				notes,
			]);
		} finally {
			rmSync(notesDir, { recursive: true, force: true });
		}
		context.releaseUrl = read('gh', [
			'release',
			'view',
			context.tag,
			'--json',
			'url',
			'-q',
			'.url',
		]);
	},
	'compare asset': ({ tag, dmgSha }) => {
		const downloads = mkdtempSync(path.join(os.tmpdir(), 'abb-release-asset-'));
		try {
			run('gh', ['release', 'download', tag, '--dir', downloads, '--pattern', '*.dmg']);
			const assets = readdirSync(downloads);
			if (assets.length !== 1) {
				throw new Error(`Release ${tag} holds ${assets.length} DMG assets; expected 1.`);
			}
			const published = sha256(path.join(downloads, assets[0]));
			if (published !== dmgSha) {
				throw new Error(`The published DMG (${published}) is not the tested DMG (${dmgSha}).`);
			}
			console.log(`The published ${assets[0]} is the tested DMG.`);
		} finally {
			rmSync(downloads, { recursive: true, force: true });
		}
	},
	'check remote parity': ({ tag, releaseSha }) => {
		const pairs = [
			['main', read('git', ['rev-parse', 'HEAD']), remoteRef('refs/heads/main')],
			[tag, read('git', ['rev-parse', `refs/tags/${tag}`]), remoteRef(`refs/tags/${tag}`)],
		];
		for (const [name, local, remote] of pairs) {
			if (local !== remote || local !== releaseSha) {
				throw new Error(
					`${name} differs: local ${local}, origin ${remote}, released ${releaseSha}.`,
				);
			}
		}
		console.log(`main and ${tag} are ${releaseSha} locally and on origin.`);
	},
	clean: () => run('bun', ['run', 'clean']),
};

function main(): void {
	if (process.argv.includes('--help') || process.argv.includes('-h')) {
		console.log(USAGE);
		return;
	}
	const repoRoot = path.resolve(import.meta.dirname, '..');
	process.chdir(repoRoot);
	const release = parseArgs(process.argv.slice(2));
	console.log('phase: preconditions');
	const problems = preconditionProblems(repoRoot, release, new Date());
	if (problems.length > 0) {
		throw new Error(`Release refused before any change:\n- ${problems.join('\n- ')}`);
	}
	const context: Context = {
		repoRoot,
		release,
		tag: `v${release.version}`,
		notes: changelogSection(
			readFileSync(path.join(repoRoot, 'CHANGELOG.md'), 'utf8'),
			release.version,
			localDate(new Date()),
		),
	};
	for (const step of stepsFor(release.lane)) {
		console.log(`phase: ${step}`);
		try {
			STEP_ACTIONS[step](context);
		} catch (error) {
			const message = error instanceof Error ? error.message : String(error);
			throw new Error(`Release stopped at '${step}':\n${message}`, { cause: error });
		}
	}
	console.log(
		[
			`phase: done (${release.lane} ${context.tag})`,
			context.dmg && `DMG ${context.dmg} SHA-256 ${context.dmgSha}`,
			installs(release.lane) && `Installed ${INSTALLED_APP}`,
			context.releaseSha && `Released commit ${context.releaseSha}`,
			context.releaseUrl && `GitHub Release ${context.releaseUrl}`,
		]
			.filter(Boolean)
			.join('\n'),
	);
}

if (import.meta.main) {
	try {
		main();
	} catch (error) {
		console.error(`Error: ${error instanceof Error ? error.message : String(error)}`);
		process.exit(1);
	}
}
