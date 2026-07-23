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
    readonly rton_worker_batch: (a: any) => [number, number, number];
    readonly rton_worker_hex_search: (a: any) => [number, number, number];
    readonly rton_worker_locate_text: (a: any) => [number, number, number];
    readonly rton_worker_mode_switch: (a: any) => [number, number, number];
    readonly rton_worker_open_text: (a: any) => [number, number, number];
    readonly rton_worker_parse: (a: any) => [number, number, number];
    readonly rton_worker_release_document: (a: any) => [number, number, number];
    readonly rton_worker_rton_size: (a: any) => [number, number, number];
    readonly rton_worker_runtime_info: () => [number, number, number];
    readonly rton_worker_text_search: (a: any) => [number, number, number];
    readonly rton_worker_text_surface: (a: any) => [number, number, number];
    readonly rton_worker_tree: (a: any) => [number, number, number];
    readonly rton_worker_value_search: (a: any) => [number, number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
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
