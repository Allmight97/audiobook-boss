import { spawnSync } from 'node:child_process';
import {
	chmodSync,
	copyFileSync,
	existsSync,
	mkdirSync,
	readdirSync,
	statSync,
	unlinkSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export const aaxcleanHelperBaseName = 'abb-aaxclean-helper';
export const aaxcleanHelperTargetTriple = 'aarch64-apple-darwin';

interface AaxcleanHelperPaths {
	projectPath: string;
	publishDir: string;
	publishedExecutablePath: string;
	sidecarDir: string;
	sidecarPath: string;
}

interface PublishAaxcleanHelperOptions {
	commandRunner?: typeof spawnSync;
	force?: boolean;
}

export function resolveDotnetCommand(): string {
	if (process.env.DOTNET_CLI && process.env.DOTNET_CLI.length > 0) {
		return process.env.DOTNET_CLI;
	}

	const userLocalDotnet = path.join(os.homedir(), '.dotnet', 'dotnet');
	if (existsSync(userLocalDotnet)) {
		return userLocalDotnet;
	}

	return 'dotnet';
}

export function resolveAaxcleanHelperPaths(repoRoot: string): AaxcleanHelperPaths {
	const projectPath = path.join(
		repoRoot,
		'tools/abb-aaxclean-helper/src/AbbAaxcleanHelper/AbbAaxcleanHelper.csproj',
	);
	const publishDir = path.join(repoRoot, 'tools/abb-aaxclean-helper/.publish/osx-arm64');
	const sidecarDir = path.join(repoRoot, 'src-tauri/binaries');
	return {
		projectPath,
		publishDir,
		publishedExecutablePath: path.join(publishDir, aaxcleanHelperBaseName),
		sidecarDir,
		sidecarPath: path.join(sidecarDir, `${aaxcleanHelperBaseName}-${aaxcleanHelperTargetTriple}`),
	};
}

export function publishAaxcleanHelper(
	repoRoot: string,
	options: PublishAaxcleanHelperOptions = {},
): string {
	const paths = resolveAaxcleanHelperPaths(repoRoot);
	if (!options.force && helperSidecarIsFresh(repoRoot, paths.sidecarPath)) {
		console.log(`[aaxclean-helper] Using existing ${paths.sidecarPath}`);
		return paths.sidecarPath;
	}

	if (existsSync(paths.publishedExecutablePath)) {
		unlinkSync(paths.publishedExecutablePath);
	}

	const dotnet = resolveDotnetCommand();
	const result = (options.commandRunner ?? spawnSync)(
		dotnet,
		[
			'publish',
			paths.projectPath,
			'-c',
			'Release',
			'-f',
			'net8.0',
			'-r',
			'osx-arm64',
			'--self-contained',
			'true',
			'-p:PublishSingleFile=true',
			'-p:PublishTrimmed=false',
			'-o',
			paths.publishDir,
		],
		{
			cwd: repoRoot,
			stdio: 'inherit',
		},
	);

	if (result.error) {
		throw result.error;
	}
	if (result.status !== 0) {
		const detail =
			typeof result.status === 'number'
				? `status ${result.status}`
				: result.signal
					? `signal ${result.signal}`
					: 'no successful exit status';
		throw new Error(`AAXClean helper publish failed (${detail})`);
	}
	if (!existsSync(paths.publishedExecutablePath)) {
		throw new Error(`Expected published AAXClean helper at ${paths.publishedExecutablePath}`);
	}

	mkdirSync(paths.sidecarDir, { recursive: true });
	copyFileSync(paths.publishedExecutablePath, paths.sidecarPath);
	chmodSync(paths.sidecarPath, 0o755);
	console.log(`[aaxclean-helper] Published ${paths.sidecarPath}`);
	return paths.sidecarPath;
}

export function verifyAaxcleanHelperSidecar(repoRoot: string): void {
	const { sidecarPath } = resolveAaxcleanHelperPaths(repoRoot);
	if (!existsSync(sidecarPath)) {
		throw new Error(`Expected AAXClean helper sidecar at ${sidecarPath}`);
	}
}

function helperSidecarIsFresh(repoRoot: string, sidecarPath: string): boolean {
	if (!existsSync(sidecarPath)) {
		return false;
	}
	const sidecarMtime = statSync(sidecarPath).mtimeMs;
	return latestHelperSourceMtime(repoRoot) <= sidecarMtime;
}

function latestHelperSourceMtime(repoRoot: string): number {
	const helperRoot = path.join(repoRoot, 'tools/abb-aaxclean-helper');
	let latest = 0;
	for (const filePath of walk(helperRoot)) {
		if (!helperSourceAffectsPublish(filePath)) {
			continue;
		}
		latest = Math.max(latest, statSync(filePath).mtimeMs);
	}
	return latest;
}

function helperSourceAffectsPublish(filePath: string): boolean {
	return (
		filePath.endsWith('.cs') ||
		filePath.endsWith('.csproj') ||
		filePath.endsWith('.props') ||
		filePath.endsWith('.targets')
	);
}

function* walk(directory: string): Generator<string> {
	for (const entry of readdirSync(directory, { withFileTypes: true })) {
		const fullPath = path.join(directory, entry.name);
		if (entry.isDirectory()) {
			if (entry.name === 'bin' || entry.name === 'obj' || entry.name === '.publish') {
				continue;
			}
			yield* walk(fullPath);
		} else {
			yield fullPath;
		}
	}
}

if (import.meta.main) {
	const repoRoot = path.resolve(import.meta.dir, '..');
	publishAaxcleanHelper(repoRoot);
}
