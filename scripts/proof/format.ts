import type { ProofStep } from './types';

type EnvLookup = Record<string, string | undefined>;

function shellQuote(value: string): string {
	if (/^[A-Za-z0-9_./:=@%+,-]+$/.test(value)) {
		return value;
	}

	return `'${value.replaceAll("'", "'\\''")}'`;
}

function formatRequiredEnv(name: string, env: EnvLookup): string {
	const value = env[name];
	if (!value) {
		return `${name}=<required>`;
	}

	return `${name}=${shellQuote(value)}`;
}

export function formatCommand(step: ProofStep, env: EnvLookup = process.env): string {
	const envPrefix = step.requiredEnv?.map((name) => formatRequiredEnv(name, env)) ?? [];
	const command = [step.command, ...step.args].map(shellQuote);
	return [...envPrefix, ...command].join(' ');
}
