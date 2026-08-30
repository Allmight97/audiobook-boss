import type { InputOwner } from '../inputSession/owner';

let boundInput: InputOwner | undefined;

export function bindProcessingInput(input: InputOwner | undefined): void {
	boundInput = input;
}

export function boundProcessingInput(): InputOwner | undefined {
	return boundInput;
}
