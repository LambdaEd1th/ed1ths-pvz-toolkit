/* tslint:disable */
/* eslint-disable */

export function rton_worker_batch(request: any): any;

export function rton_worker_hex_search(request: any): any;

export function rton_worker_locate_text(request: any): any;

export function rton_worker_mode_switch(request: any): any;

export function rton_worker_open_text(request: any): any;

export function rton_worker_parse(request: any): any;

export function rton_worker_release_document(request: any): any;

export function rton_worker_rton_size(request: any): any;

export function rton_worker_runtime_info(): any;

export function rton_worker_text_search(request: any): any;

export function rton_worker_text_surface(request: any): any;

export function rton_worker_tree(request: any): any;

export function rton_worker_value_search(request: any): any;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly rton_worker_batch: (a: number, b: number) => void;
    readonly rton_worker_hex_search: (a: number, b: number) => void;
    readonly rton_worker_locate_text: (a: number, b: number) => void;
    readonly rton_worker_mode_switch: (a: number, b: number) => void;
    readonly rton_worker_open_text: (a: number, b: number) => void;
    readonly rton_worker_parse: (a: number, b: number) => void;
    readonly rton_worker_release_document: (a: number, b: number) => void;
    readonly rton_worker_rton_size: (a: number, b: number) => void;
    readonly rton_worker_runtime_info: (a: number) => void;
    readonly rton_worker_text_search: (a: number, b: number) => void;
    readonly rton_worker_text_surface: (a: number, b: number) => void;
    readonly rton_worker_tree: (a: number, b: number) => void;
    readonly rton_worker_value_search: (a: number, b: number) => void;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number) => void;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
