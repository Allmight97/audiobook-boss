type NullableKeys<T> = {
	[K in keyof T]-?: null extends T[K] ? K : never;
}[keyof T];

type NonNullableKeys<T> = Exclude<keyof T, NullableKeys<T>>;

type Simplify<T> = { [K in keyof T]: T[K] } & {};

// Converts generated `T | null` payload fields into UI-friendly optional fields.
export type NullToOptionalDeep<T> = T extends readonly (infer U)[]
	? NullToOptionalDeep<U>[]
	: T extends object
		? Simplify<
				{
					[K in NonNullableKeys<T>]: NullToOptionalDeep<T[K]>;
				} & {
					[K in NullableKeys<T>]?: NullToOptionalDeep<Exclude<T[K], null>>;
				}
			>
		: T;
