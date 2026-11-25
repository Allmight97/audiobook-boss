import { createHash } from 'node:crypto';
import { spawn, spawnSync } from 'node:child_process';
import { readFileSync, realpathSync } from 'node:fs';
import path from 'node:path';

interface DevelopmentIdentity {
	identifier: string;
	productName: string;
}

type GitRunner = (args: string[]) => { status: number | null; stdout: string };

interface GitCheckoutState {
	fullRevision: string;
	dirty: boolean | 'unknown';
	buildRevision: string;
}

export function developmentIdentity(repoRoot: string): DevelopmentIdentity {
	const checkout = realpathSync(repoRoot);
	const config: DevelopmentIdentity = JSON.parse(
		readFileSync(path.join(checkout, 'src-tauri/tauri.conf.json'), 'utf8'),
	);
	const checkoutId = createHash('sha256').update(checkout).digest('hex').slice(0, 12);
	return {
		identifier: `${config.identifier}.dev.${checkoutId}`,
		productName: `${config.productName} Dev ${checkoutId.slice(0, 6)}`,
	};
}

export function hasDebugFlag(cliArgs: string[]): boolean {
	return cliArgs.includes('--debug') || cliArgs.includes('-d');
}

export function withDevelopmentIdentity(repoRoot: string, args: string[]): string[] {
	const boundary = args.indexOf('--');
	const cliArgs = boundary < 0 ? args : args.slice(0, boundary);
	const command = cliArgs.find((arg) => !arg.startsWith('-'));
	const debugBuild = command === 'build' && hasDebugFlag(cliArgs);
	if (command !== 'dev' && !debugBuild) return args;

	return [
		...cliArgs,
		'--config',
		JSON.stringify(developmentIdentity(repoRoot)),
		...(boundary < 0 ? [] : args.slice(boundary)),
	];
}

const runGit: GitRunner = (args) => {
	const result = spawnSync('git', args, { encoding: 'utf8' });
	return { status: result.status, stdout: result.stdout ?? '' };
};

function readCheckoutState(git: GitRunner = runGit): GitCheckoutState {
	const head = git(['rev-parse', 'HEAD']);
	const status = git(['status', '--porcelain']);
	const fullRevision = head.status === 0 ? head.stdout.trim() : 'unknown';
	const dirty = status.status === 0 ? Boolean(status.stdout.trim()) : 'unknown';
	const revision =
		head.status !== 0 || status.status !== 0
			? 'unknown'
			: dirty
				? `${fullRevision.slice(0, 12)}-dirty`
				: fullRevision.slice(0, 12);
	return { fullRevision, dirty, buildRevision: revision };
}

export function buildRevision(git: GitRunner = runGit): string {
	return readCheckoutState(git).buildRevision;
}

async function runTauriCli(tauriBin: string, args: string[], revision: string): Promise<number> {
	const child = spawn(process.execPath, [tauriBin, ...args], {
		stdio: 'inherit',
		env: { ...process.env, ABB_BUILD_REVISION: revision },
	});
	const ignoreSigint = () => {};
	const forwardTerm = () => child.kill('SIGTERM');
	const forwardHup = () => child.kill('SIGHUP');
	process.on('SIGINT', ignoreSigint);
	process.on('SIGTERM', forwardTerm);
	process.on('SIGHUP', forwardHup);
	try {
		return await new Promise<number>((resolve) => {
			child.on('error', (error) => {
				console.error(error.message);
				resolve(1);
			});
			child.on('exit', (code, signal) => {
				if (code !== null) {
					resolve(code);
				} else {
					resolve(signal === 'SIGINT' ? 130 : signal === 'SIGHUP' ? 129 : 143);
				}
			});
		});
	} finally {
		process.off('SIGINT', ignoreSigint);
		process.off('SIGTERM', forwardTerm);
		process.off('SIGHUP', forwardHup);
	}
}

if (import.meta.main) {
	const args = withDevelopmentIdentity(process.cwd(), process.argv.slice(2));
	const state = readCheckoutState();
	console.info(
		`build_checkout revision=${state.fullRevision} dirty=${state.dirty} checkout=${JSON.stringify(path.basename(realpathSync(process.cwd())))}`,
	);
	const tauriBin = path.join(import.meta.dir, '..', 'node_modules/.bin/tauri');
	process.exitCode = await runTauriCli(tauriBin, args, state.buildRevision);
}
