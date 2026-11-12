/**
 * What changed in FFmpeg master since ABB's pinned commit, in the sources
 * ABB's build can compile.
 *
 * Usage: bun run ffmpeg:upstream [--issue]
 *        bun run ffmpeg:pin <commit>
 *
 * Reads the pin from `vendor/ffmpeg-sys-next-9.0.1/ffmpeg-revision` and the
 * built components (the lists in that crate's `build.rs`, `CONFIGURE_SELECTED`
 * included). Fetches master into a blobless clone under
 * `target/abb-ffmpeg-upstream/` (reused across runs) and lists the commits
 * after the pin that touch `configure`, libavcodec, libavformat, libavutil or
 * libswresample, leaving out a source file that the pin's and master's
 * Makefiles assign only to components ABB does not build. Prints a Markdown
 * report. With `--issue` it instead keeps one open issue current through
 * `gh api` (REST only; cloud sessions block GraphQL): the issue exists while
 * relevant commits exist, and its body changes only when the pin or the
 * commit list changes. The scheduled workflow
 * `.github/workflows/ffmpeg-upstream.yml` runs it.
 *
 * `ffmpeg:pin` (`--pin <commit>`) moves every pin of the FFmpeg commit at
 * once: the full hash into `ffmpeg-revision`, and `ffmpeg_cli_release` and
 * `ffmpeg_cli_build` in `scripts/setup.sh` to the newest FFmpeg-Builds master
 * autobuild of that commit. It changes nothing when no autobuild names the
 * commit, and lists the commits that have one.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const SYS_CRATE = 'vendor/ffmpeg-sys-next-9.0.1';
const UPSTREAM_URL = 'https://github.com/FFmpeg/FFmpeg.git';
export const ISSUE_TITLE = 'FFmpeg upstream: changes since the pin';
const NEXT_ACTION =
	'Next action: decide whether to move the pin (procedure: vendor/ffmpeg-sys-next-9.0.1/ABB-PROVENANCE.md, Moving the pin).';
const CACHE_DIRECTORY = 'target/abb-ffmpeg-upstream';
const SETUP_SH = 'scripts/setup.sh';
const AUTOBUILD_RELEASES = 'repos/FFmpeg-Builds/FFmpeg-Builds/releases?per_page=100';

const WATCHED_ROOTS = ['configure', 'libavcodec', 'libavformat', 'libavutil', 'libswresample'];
const COMPONENT_LIBRARIES = ['libavcodec', 'libavformat'];
const REGISTRIES = [
	'libavcodec/allcodecs.c',
	'libavcodec/bitstream_filters.c',
	'libavcodec/hwaccels.h',
	'libavcodec/parsers.c',
	'libavformat/allformats.c',
	'libavformat/protocols.c',
];
const SOURCE_EXTENSIONS = new Set(['.c', '.S', '.asm']);

export interface Component {
	kind: string;
	name: string;
}

export interface Commit {
	hash: string;
	date: string;
	subject: string;
}

export interface Makefile {
	library: string;
	text: string;
}

export interface Report {
	pin: string;
	head: string;
	commits: Commit[];
}

export function parseComponents(buildRs: string): Component[] {
	const components = [...buildRs.matchAll(/\(\s*"([a-z]+)",\s*&\[([^\]]*)\]/g)].flatMap(
		([, kind, names]) => [...names.matchAll(/"([a-z0-9_]+)"/g)].map(([, name]) => ({ kind, name })),
	);
	if (components.length === 0) throw new Error('build.rs lists no components.');
	return components;
}

/**
 * The condition of every Makefile line that compiles each object:
 * `OBJS-$(CONFIG_<NAME>_<KIND>) += a.o`, its assembly and SIMD variants, and
 * `OBJS += a.o` (condition ''), with `\` continuations. Object paths are
 * relative to the library directory; keys are `<library>/<object without .o>`.
 */
export function objectConditions(makefiles: Makefile[]): Map<string, Set<string>> {
	const conditions = new Map<string, Set<string>>();
	for (const { library, text } of makefiles) {
		for (const line of text.replace(/\\\r?\n/g, ' ').split('\n')) {
			const match = /^(?:[A-Z0-9]+-)?(?:SHLIB|STLIB)?OBJS(?:-\$\(([^)]*)\))?\s*\+?=(.*)$/.exec(
				line,
			);
			if (!match) continue;
			for (const object of match[2].split(/\s+/).filter((word) => word.endsWith('.o'))) {
				const key = `${library}/${object.slice(0, -2)}`;
				const set = conditions.get(key) ?? new Set<string>();
				set.add(match[1] ?? '');
				conditions.set(key, set);
			}
		}
	}
	return conditions;
}

/**
 * The `CONFIG_` name of each component FFmpeg registers (`extern const FFCodec
 * ff_aac_decoder;` is `CONFIG_AAC_DECODER`), as `configure` finds them.
 */
export function registeredComponents(registries: string[]): Set<string> {
	return new Set(
		registries.flatMap((text) =>
			[
				...text.matchAll(
					/\bff_(\w+?_(?:bsf|decoder|demuxer|encoder|hwaccel|muxer|parser|protocol))\b/g,
				),
			].map(([, component]) => `CONFIG_${component.toUpperCase()}`),
		),
	);
}

/**
 * Source files compiled only for registered components ABB does not build.
 * An object stays watched when any line compiles it unconditionally, for a
 * built component, or for a subsystem such as `CONFIG_FRAME_THREAD_ENCODER`.
 */
export function unbuiltSources(
	built: Component[],
	registries: string[],
	makefiles: Makefile[],
	files: Iterable<string>,
): string[] {
	const components = registeredComponents(registries);
	for (const { kind, name } of built) {
		components.delete(`CONFIG_${name.toUpperCase()}_${kind.toUpperCase()}`);
	}
	const unbuilt = new Set<string>();
	for (const [object, conditions] of objectConditions(makefiles)) {
		if ([...conditions].every((when) => components.has(when))) unbuilt.add(object);
	}
	return [...files]
		.filter((file) => {
			const extension = path.extname(file);
			return SOURCE_EXTENSIONS.has(extension) && unbuilt.has(file.slice(0, -extension.length));
		})
		.toSorted();
}

export function watchedPathspecs(unbuilt: string[]): string[] {
	return [...WATCHED_ROOTS, ...unbuilt.map((file) => `:(exclude)${file}`)];
}

export function parseCommits(log: string): Commit[] {
	return log
		.split('\n')
		.filter((line) => line.length > 0)
		.map((line) => {
			const [hash = '', date = '', ...subject] = line.split('\t');
			return { hash, date, subject: subject.join('\t') };
		});
}

export function renderReport(report: Report): string {
	const lines = [
		'# FFmpeg upstream watch',
		'',
		`Pinned: \`${report.pin.slice(0, 10)}\`. Master head: \`${report.head.slice(0, 10)}\`.`,
		'',
	];
	if (report.commits.length === 0) {
		lines.push('no relevant upstream changes');
	} else {
		lines.push(
			`${report.commits.length} commit(s) after the pin touch sources ABB's build can compile:`,
			'',
			'| Commit | Date | Subject |',
			'| --- | --- | --- |',
			...report.commits.map(
				(commit) =>
					`| \`${commit.hash.slice(0, 10)}\` | ${commit.date} | ${commit.subject.replaceAll('|', '\\|')} |`,
			),
		);
	}
	return lines.join('\n');
}

function run(file: string, args: string[], cwd?: string, input?: string): string {
	return execFileSync(file, args, {
		cwd,
		input,
		encoding: 'utf8',
		maxBuffer: 64 * 1024 * 1024,
		stdio: [input === undefined ? 'ignore' : 'pipe', 'pipe', 'inherit'],
	}).trim();
}

function ensureClone(cache: string): void {
	if (existsSync(path.join(cache, 'HEAD'))) {
		run('git', ['fetch', '--quiet', 'origin', 'master:refs/heads/master'], cache);
		return;
	}
	run('git', [
		'clone',
		'--quiet',
		'--bare',
		'--filter=blob:none',
		'--single-branch',
		'--branch=master',
		UPSTREAM_URL,
		cache,
	]);
}

function readMakefiles(
	cache: string,
	revisions: string[],
): { registries: string[]; makefiles: Makefile[]; files: Set<string> } {
	const registries: string[] = [];
	const makefiles: Makefile[] = [];
	const files = new Set<string>();
	for (const revision of revisions) {
		for (const registry of REGISTRIES) {
			registries.push(run('git', ['show', `${revision}:${registry}`], cache));
		}
		const tree = run(
			'git',
			['ls-tree', '-r', '--name-only', revision, '--', ...COMPONENT_LIBRARIES],
			cache,
		)
			.split('\n')
			.filter((file) => file.length > 0);
		for (const file of tree) files.add(file);
		for (const file of tree.filter((name) => name.endsWith('/Makefile'))) {
			makefiles.push({
				library: file.split('/')[0],
				text: run('git', ['show', `${revision}:${file}`], cache),
			});
		}
	}
	return { registries, makefiles, files };
}

export type Gh = (args: string[], input?: string) => string;

export interface OpenIssue {
	number: number;
	title: string;
	body: string;
}

export type IssueAction =
	| { kind: 'create'; body: string }
	| { kind: 'update'; number: number; body: string }
	| { kind: 'close'; number: number }
	| { kind: 'unchanged'; number: number }
	| { kind: 'none' };

export function issueBody(report: Report, checkedAt: Date): string {
	const stamp = checkedAt.toISOString().slice(0, 16).replace('T', ' ');
	return [
		`Checked ${stamp} UTC against pin ${report.pin.slice(0, 10)}.`,
		NEXT_ACTION,
		renderReport(report),
	].join('\n\n');
}

function listedState(body: string): { pin: string | undefined; commits: string[] } {
	return {
		pin: /^Checked .* against pin (\w+)\./.exec(body)?.[1],
		commits: [...body.matchAll(/^\| `(\w+)` \|/gm)].map(([, hash]) => hash),
	};
}

export function decideIssueAction(
	report: Report,
	issue: OpenIssue | undefined,
	checkedAt: Date,
): IssueAction {
	if (report.commits.length === 0) {
		return issue ? { kind: 'close', number: issue.number } : { kind: 'none' };
	}
	const body = issueBody(report, checkedAt);
	if (!issue) return { kind: 'create', body };
	const listed = listedState(issue.body);
	const current = listedState(body);
	const same =
		listed.pin === current.pin &&
		listed.commits.length === current.commits.length &&
		listed.commits.every((hash, index) => hash === current.commits[index]);
	return same
		? { kind: 'unchanged', number: issue.number }
		: { kind: 'update', number: issue.number, body };
}

export function syncIssue(report: Report, gh: Gh, slug: string, checkedAt: Date): string {
	const openIssues = gh([
		'api',
		'--paginate',
		`repos/${slug}/issues?state=open&per_page=100`,
		'--jq',
		'.[] | select(.pull_request == null) | {number, title, body: (.body // "")}',
	])
		.split('\n')
		.filter((line) => line.length > 0)
		.map((line) => JSON.parse(line) as OpenIssue);
	const issue = openIssues.find((candidate) => candidate.title === ISSUE_TITLE);
	const action = decideIssueAction(report, issue, checkedAt);
	switch (action.kind) {
		case 'create': {
			const created = JSON.parse(
				gh(
					['api', '--method', 'POST', `repos/${slug}/issues`, '--input', '-'],
					JSON.stringify({ title: ISSUE_TITLE, body: action.body }),
				),
			) as { number: number };
			return `created issue #${created.number}`;
		}
		case 'update':
			gh(
				['api', '--method', 'PATCH', `repos/${slug}/issues/${action.number}`, '--input', '-'],
				JSON.stringify({ body: action.body }),
			);
			return `updated issue #${action.number}`;
		case 'close':
			gh(
				['api', '--method', 'PATCH', `repos/${slug}/issues/${action.number}`, '--input', '-'],
				JSON.stringify({ state: 'closed', state_reason: 'completed' }),
			);
			return `closed issue #${action.number}`;
		case 'unchanged':
			return `issue #${action.number} already lists these commits`;
		case 'none':
			return 'no relevant upstream changes and no open issue';
	}
}

function repoSlug(): string {
	return (
		process.env.GITHUB_REPOSITORY ||
		run('gh', ['repo', 'view', '--json', 'nameWithOwner', '--jq', '.nameWithOwner'])
	);
}

function buildReport(repoRoot: string): Report {
	const pin = readFileSync(path.join(repoRoot, SYS_CRATE, 'ffmpeg-revision'), 'utf8').trim();
	const buildRs = readFileSync(path.join(repoRoot, SYS_CRATE, 'build.rs'), 'utf8');
	const cache = path.join(repoRoot, CACHE_DIRECTORY, 'FFmpeg.git');
	ensureClone(cache);

	const { registries, makefiles, files } = readMakefiles(cache, [pin, 'master']);
	const unbuilt = unbuiltSources(parseComponents(buildRs), registries, makefiles, files);
	const head = run('git', ['rev-parse', 'master'], cache);
	const commits = parseCommits(
		run(
			'git',
			['log', '--format=%H%x09%cs%x09%s', `${pin}..master`, '--', ...watchedPathspecs(unbuilt)],
			cache,
		),
	);
	return { pin, head, commits };
}

export interface Release {
	tag_name: string;
	assets: string[];
}

export interface Autobuild {
	release: string;
	build: string;
}

/**
 * The master autobuilds setup can install, newest first: an `autobuild-`
 * release with the checksums and both Linux GPL archives of one describe id
 * (`N-<count>-g<short hash>`).
 */
export function masterAutobuilds(releases: Release[]): Autobuild[] {
	return releases.flatMap(({ tag_name: release, assets }) => {
		if (!release.startsWith('autobuild-') || !assets.includes('checksums.sha256')) return [];
		return assets.flatMap((asset) => {
			const build = /^ffmpeg-(N-\d+-g[0-9a-f]+)-linux64-gpl\.tar\.xz$/.exec(asset)?.[1];
			return build && assets.includes(`ffmpeg-${build}-linuxarm64-gpl.tar.xz`)
				? [{ release, build }]
				: [];
		});
	});
}

function shortHash(build: string): string {
	return build.slice(build.lastIndexOf('-g') + 2);
}

export function withCliPins(setupSh: string, autobuild: Autobuild): string {
	let text = setupSh;
	for (const [name, value] of [
		['ffmpeg_cli_release', autobuild.release],
		['ffmpeg_cli_build', autobuild.build],
	]) {
		const line = new RegExp(`^${name}=\\S*$`, 'm');
		if (!line.test(text)) throw new Error(`${SETUP_SH} has no ${name}= line.`);
		text = text.replace(line, `${name}=${value}`);
	}
	return text;
}

export function movePin(repoRoot: string, commitish: string, gh: Gh): string {
	if (!/^[0-9a-f]{7,40}$/.test(commitish)) {
		throw new Error(`ffmpeg:pin needs an FFmpeg commit hash, got '${commitish}'.`);
	}
	const commit = gh(['api', `repos/FFmpeg/FFmpeg/commits/${commitish}`, '--jq', '.sha']);
	const autobuilds = masterAutobuilds(
		gh([
			'api',
			'--paginate',
			AUTOBUILD_RELEASES,
			'--jq',
			'.[] | {tag_name, assets: [.assets[].name]}',
		])
			.split('\n')
			.filter((line) => line.length > 0)
			.map((line) => JSON.parse(line) as Release),
	);
	const autobuild = autobuilds.find(({ build }) => commit.startsWith(shortHash(build)));
	if (!autobuild) {
		const built = autobuilds.map(({ build }) => shortHash(build)).join(', ');
		throw new Error(
			`No FFmpeg-Builds master autobuild names ${commit.slice(0, 10)}. Commits that have one: ${built}.`,
		);
	}
	const setupSh = path.join(repoRoot, SETUP_SH);
	const pinned = withCliPins(readFileSync(setupSh, 'utf8'), autobuild);
	writeFileSync(path.join(repoRoot, SYS_CRATE, 'ffmpeg-revision'), `${commit}\n`);
	writeFileSync(setupSh, pinned);
	return `Pinned FFmpeg ${commit}: ffmpeg_cli_release=${autobuild.release} ffmpeg_cli_build=${autobuild.build}`;
}

if (import.meta.main) {
	try {
		const [flag, value, ...extra] = process.argv.slice(2);
		const repoRoot = path.resolve(import.meta.dirname, '..');
		const gh: Gh = (args, input) => run('gh', args, undefined, input);
		if (flag === '--pin' && value !== undefined && extra.length === 0) {
			console.log(movePin(repoRoot, value, gh));
		} else if (flag === undefined || (flag === '--issue' && value === undefined)) {
			const report = buildReport(repoRoot);
			console.log(
				flag === '--issue' ? syncIssue(report, gh, repoSlug(), new Date()) : renderReport(report),
			);
		} else {
			throw new Error(`Unknown arguments: ${process.argv.slice(2).join(' ')}`);
		}
	} catch (error) {
		console.error(`Error: ${error instanceof Error ? error.message : String(error)}`);
		process.exit(1);
	}
}
