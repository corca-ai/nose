export function pureCallbackMap(xs: number[]): number[] {
    return xs.map((x) => x);
}

export function pureCallbackLoop(xs: number[]): number[] {
    const out: number[] = [];
    for (const x of xs) {
        out.push(x);
    }
    return out;
}

export function observedCallbackMap(xs: number[]): number[] {
    return xs.map((x) => {
        console.log(x);
        return x;
    });
}

export function capturedMutationCallbackMap(xs: number[]): number[] {
    let offset = 0;
    return xs.map((x) => {
        offset += 1;
        return x * 2 + offset;
    });
}

export function extraArgumentCallbackMap(xs: number[]): number[] {
    return xs.map((x, index) => x * 2 + index);
}

export function customDispatchMap(xs: number[]): number[] {
    const custom = {
        map(callback: (value: number, index: number) => number): number[] {
            return xs.map((value, index) => callback(value, index));
        },
    };
    return custom.map((x, index) => x * 2 + index);
}

export function throwingCallbackMap(xs: number[]): number[] {
    return xs.map((x) => {
        if (x < 0) {
            throw new Error("negative");
        }
        return x;
    });
}

export function overloadableOperatorCallbackMap(xs: unknown[]): unknown[] {
    return xs.map((x: any) => x + 1);
}

function observeAndReturn(x: number): number {
    console.log(x);
    return x;
}

export function nestedReceiverEffectCallbackMap(xs: number[]): number[][] {
    return xs.map((x: number) => [observeAndReturn(x)].map((y: number) => y));
}

export function implicitArgumentsCallbackMap(xs: number[]): IArguments[] {
    return xs.map(function (x: number) {
        return arguments;
    });
}

function observeDefault(): number {
    console.log("default");
    return 0;
}

export function defaultParameterCallbackMap(xs: number[]): number[] {
    return xs.map((x = observeDefault()) => x);
}

export function restParameterCallbackMap(xs: number[]): unknown[][] {
    return xs.map((...values) => values);
}

export function destructuredParameterCallbackMap(xs: Array<[number]>): number[] {
    return xs.map(([value]) => value);
}

export function mixedBigIntCallbackMap(xs: number[]): unknown[] {
    return xs.map((_x) => 1n + 1);
}

export function equalityCallbackMap(xs: number[]): boolean[] {
    return xs.map((_x) => 1 === 2);
}

export function instanceofTrapCallbackMap(xs: number[]): boolean[] {
    return xs.map((_x) => 1 instanceof (2 as any));
}

export function wrappedSourceCallbackMap(xs: number[], source: number[]): number[][][] {
    return xs.map((_x) => [source]);
}

export function arraySpreadCallbackMap(xs: number[], source: number[]): number[][] {
    return xs.map((_x) => [...source]);
}

declare const unresolvedCallbackValue: unknown;

export function localReadCallbackMap(xs: unknown[], value: unknown): unknown[] {
    return xs.map((_x) => value);
}

export function unresolvedGlobalCallbackMap(xs: unknown[], _value: unknown): unknown[] {
    return xs.map((_x) => unresolvedCallbackValue);
}

export function constantCallbackMap(xs: number[]): number[] {
    return xs.map((_x) => 1);
}

export function negativeCallbackMap(xs: number[]): number[] {
    return xs.map((value) => -value);
}

export function negativeCallbackLoop(xs: number[]): number[] {
    const out: number[] = [];
    for (const value of xs) {
        out.push(-value);
    }
    return out;
}

export function nestedNumericPredicateCallbackMap(
    xs: number[],
    ys: number[],
): number[][] {
    return xs.map((x: number) => ys.filter((y: number) => y > x));
}

export function nestedCoercivePredicateCallbackMap(
    xs: number[],
    ys: unknown[],
): unknown[][] {
    return xs.map((x: number) => ys.filter((y: any) => y > x));
}

export function objectEntryCallbackMap(xs: number[]): Array<Record<string, number>> {
    return xs.map((value) => ({ value }));
}

export function objectEntryCallbackLoop(xs: number[]): Array<Record<string, number>> {
    const out: Array<Record<string, number>> = [];
    for (const value of xs) {
        out.push({ value });
    }
    return out;
}

class CallbackBase {}

function observeClassBase(): typeof CallbackBase {
    console.log("heritage");
    return CallbackBase;
}

export function staticClassHeritageCallbackMap(xs: number[]): unknown[][] {
    return xs.map((_value) => [class extends CallbackBase {}]);
}

export function observedClassHeritageCallbackMap(xs: number[]): unknown[][] {
    return xs.map((_value) => [class extends observeClassBase() {}]);
}
