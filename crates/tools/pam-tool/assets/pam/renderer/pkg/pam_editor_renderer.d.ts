/* tslint:disable */
/* eslint-disable */

export class RendererHandle {
    free(): void;
    [Symbol.dispose](): void;
    destroy(): void;
    frame(): void;
    constructor();
    resize(width: number, height: number, scale_factor: number): void;
    set_scene(scene: any): void;
    set_view(view: any): void;
    start(canvas: HTMLCanvasElement, width: number, height: number, scale_factor: number): Promise<void>;
    start_offscreen(canvas: OffscreenCanvas, width: number, height: number, scale_factor: number): Promise<void>;
    start_webgl(canvas: HTMLCanvasElement, width: number, height: number, scale_factor: number): Promise<void>;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_rendererhandle_free: (a: number, b: number) => void;
    readonly rendererhandle_destroy: (a: number) => void;
    readonly rendererhandle_frame: (a: number, b: number) => void;
    readonly rendererhandle_new: () => number;
    readonly rendererhandle_resize: (a: number, b: number, c: number, d: number) => void;
    readonly rendererhandle_set_scene: (a: number, b: number, c: number) => void;
    readonly rendererhandle_set_view: (a: number, b: number, c: number) => void;
    readonly rendererhandle_start: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly rendererhandle_start_offscreen: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly rendererhandle_start_webgl: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly __wasm_bindgen_func_elem_1713: (a: number, b: number, c: number, d: number) => void;
    readonly __wasm_bindgen_func_elem_1713_12: (a: number, b: number, c: number, d: number) => void;
    readonly __wasm_bindgen_func_elem_1713_13: (a: number, b: number, c: number, d: number) => void;
    readonly __wasm_bindgen_func_elem_9967: (a: number, b: number, c: number, d: number) => void;
    readonly __wasm_bindgen_func_elem_9969: (a: number, b: number, c: number, d: number) => void;
    readonly __wasm_bindgen_func_elem_1016: (a: number, b: number) => void;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number) => void;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
    readonly __wbindgen_export5: (a: number, b: number) => void;
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
