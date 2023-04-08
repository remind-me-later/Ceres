// @generated file from wasmbuild -- do not edit
// deno-lint-ignore-file
// deno-fmt-ignore-file
// source-hash: 6b9d57848648f19f3abf9494b922793c8d09a831
let wasm;

const cachedTextDecoder = new TextDecoder("utf-8", {
  ignoreBOM: true,
  fatal: true,
});

cachedTextDecoder.decode();

let cachedUint8Memory0 = null;

function getUint8Memory0() {
  if (cachedUint8Memory0 === null || cachedUint8Memory0.byteLength === 0) {
    cachedUint8Memory0 = new Uint8Array(wasm.memory.buffer);
  }
  return cachedUint8Memory0;
}

function getStringFromWasm0(ptr, len) {
  return cachedTextDecoder.decode(getUint8Memory0().subarray(ptr, ptr + len));
}
/**
 * @returns {GbHandle}
 */
export function init_emulator() {
  const ret = wasm.init_emulator();
  return GbHandle.__wrap(ret);
}

let WASM_VECTOR_LEN = 0;

function passArray8ToWasm0(arg, malloc) {
  const ptr = malloc(arg.length * 1);
  getUint8Memory0().set(arg, ptr / 1);
  WASM_VECTOR_LEN = arg.length;
  return ptr;
}
/**
 * @param {Uint8Array} rom
 * @returns {GbHandle}
 */
export function init_emulator_with_rom(rom) {
  const ptr0 = passArray8ToWasm0(rom, wasm.__wbindgen_malloc);
  const len0 = WASM_VECTOR_LEN;
  const ret = wasm.init_emulator_with_rom(ptr0, len0);
  return GbHandle.__wrap(ret);
}

function _assertClass(instance, klass) {
  if (!(instance instanceof klass)) {
    throw new Error(`expected instance of ${klass.name}`);
  }
  return instance.ptr;
}

let cachedInt32Memory0 = null;

function getInt32Memory0() {
  if (cachedInt32Memory0 === null || cachedInt32Memory0.byteLength === 0) {
    cachedInt32Memory0 = new Int32Array(wasm.memory.buffer);
  }
  return cachedInt32Memory0;
}

function getArrayU8FromWasm0(ptr, len) {
  return getUint8Memory0().subarray(ptr / 1, ptr / 1 + len);
}
/**
 * @param {GbHandle} emulator
 * @returns {Uint8Array}
 */
export function get_framebuffer(emulator) {
  try {
    const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
    _assertClass(emulator, GbHandle);
    wasm.get_framebuffer(retptr, emulator.ptr);
    var r0 = getInt32Memory0()[retptr / 4 + 0];
    var r1 = getInt32Memory0()[retptr / 4 + 1];
    var v0 = getArrayU8FromWasm0(r0, r1).slice();
    wasm.__wbindgen_free(r0, r1 * 1);
    return v0;
  } finally {
    wasm.__wbindgen_add_to_stack_pointer(16);
  }
}

/**
 * @param {GbHandle} emulator
 * @returns {AudioSamples}
 */
export function run_sample(emulator) {
  _assertClass(emulator, GbHandle);
  const ret = wasm.run_sample(emulator.ptr);
  return AudioSamples.__wrap(ret);
}

/**
 * @param {GbHandle} emulator
 * @param {number} num_samples
 */
export function run_n_samples(emulator, num_samples) {
  _assertClass(emulator, GbHandle);
  wasm.run_n_samples(emulator.ptr, num_samples);
}

/**
 * @param {GbHandle} emulator
 * @param {number} button
 */
export function press_button(emulator, button) {
  _assertClass(emulator, GbHandle);
  wasm.press_button(emulator.ptr, button);
}

/**
 * @param {GbHandle} emulator
 * @param {number} button
 */
export function release_button(emulator, button) {
  _assertClass(emulator, GbHandle);
  wasm.release_button(emulator.ptr, button);
}

const AudioSamplesFinalization = new FinalizationRegistry((ptr) =>
  wasm.__wbg_audiosamples_free(ptr)
);
/** */
export class AudioSamples {
  static __wrap(ptr) {
    const obj = Object.create(AudioSamples.prototype);
    obj.ptr = ptr;
    AudioSamplesFinalization.register(obj, obj.ptr, obj);
    return obj;
  }

  __destroy_into_raw() {
    const ptr = this.ptr;
    this.ptr = 0;
    AudioSamplesFinalization.unregister(this);
    return ptr;
  }

  free() {
    const ptr = this.__destroy_into_raw();
    wasm.__wbg_audiosamples_free(ptr);
  }
  /**
   * @returns {number}
   */
  get left() {
    const ret = wasm.__wbg_get_audiosamples_left(this.ptr);
    return ret;
  }
  /**
   * @param {number} arg0
   */
  set left(arg0) {
    wasm.__wbg_set_audiosamples_left(this.ptr, arg0);
  }
  /**
   * @returns {number}
   */
  get right() {
    const ret = wasm.__wbg_get_audiosamples_right(this.ptr);
    return ret;
  }
  /**
   * @param {number} arg0
   */
  set right(arg0) {
    wasm.__wbg_set_audiosamples_right(this.ptr, arg0);
  }
}

const GbHandleFinalization = new FinalizationRegistry((ptr) =>
  wasm.__wbg_gbhandle_free(ptr)
);
/** */
export class GbHandle {
  static __wrap(ptr) {
    const obj = Object.create(GbHandle.prototype);
    obj.ptr = ptr;
    GbHandleFinalization.register(obj, obj.ptr, obj);
    return obj;
  }

  __destroy_into_raw() {
    const ptr = this.ptr;
    this.ptr = 0;
    GbHandleFinalization.unregister(this);
    return ptr;
  }

  free() {
    const ptr = this.__destroy_into_raw();
    wasm.__wbg_gbhandle_free(ptr);
  }
}

const imports = {
  __wbindgen_placeholder__: {
    __wbindgen_throw: function (arg0, arg1) {
      throw new Error(getStringFromWasm0(arg0, arg1));
    },
  },
};

/**
 * Decompression callback
 *
 * @callback DecompressCallback
 * @param {Uint8Array} compressed
 * @return {Uint8Array} decompressed
 */

/**
 * Options for instantiating a Wasm instance.
 * @typedef {Object} InstantiateOptions
 * @property {URL=} url - Optional url to the Wasm file to instantiate.
 * @property {DecompressCallback=} decompress - Callback to decompress the
 * raw Wasm file bytes before instantiating.
 */

/** Instantiates an instance of the Wasm module returning its functions.
 * @remarks It is safe to call this multiple times and once successfully
 * loaded it will always return a reference to the same object.
 * @param {InstantiateOptions=} opts
 */
export async function instantiate(opts) {
  return (await instantiateWithInstance(opts)).exports;
}

let instanceWithExports;
let lastLoadPromise;

/** Instantiates an instance of the Wasm module along with its exports.
 * @remarks It is safe to call this multiple times and once successfully
 * loaded it will always return a reference to the same object.
 * @param {InstantiateOptions=} opts
 * @returns {Promise<{
 *   instance: WebAssembly.Instance;
 *   exports: { init_emulator: typeof init_emulator; init_emulator_with_rom: typeof init_emulator_with_rom; get_framebuffer: typeof get_framebuffer; run_sample: typeof run_sample; run_n_samples: typeof run_n_samples; press_button: typeof press_button; release_button: typeof release_button; AudioSamples : typeof AudioSamples ; GbHandle : typeof GbHandle  }
 * }>}
 */
export function instantiateWithInstance(opts) {
  if (instanceWithExports != null) {
    return Promise.resolve(instanceWithExports);
  }
  if (lastLoadPromise == null) {
    lastLoadPromise = (async () => {
      try {
        const instance = (await instantiateModule(opts ?? {})).instance;
        wasm = instance.exports;
        cachedInt32Memory0 = new Int32Array(wasm.memory.buffer);
        cachedUint8Memory0 = new Uint8Array(wasm.memory.buffer);
        instanceWithExports = {
          instance,
          exports: getWasmInstanceExports(),
        };
        return instanceWithExports;
      } finally {
        lastLoadPromise = null;
      }
    })();
  }
  return lastLoadPromise;
}

function getWasmInstanceExports() {
  return {
    init_emulator,
    init_emulator_with_rom,
    get_framebuffer,
    run_sample,
    run_n_samples,
    press_button,
    release_button,
    AudioSamples,
    GbHandle,
  };
}

/** Gets if the Wasm module has been instantiated. */
export function isInstantiated() {
  return instanceWithExports != null;
}

/**
 * @param {InstantiateOptions} opts
 */
async function instantiateModule(opts) {
  const wasmUrl = opts.url ?? new URL("rs_lib_bg.wasm", import.meta.url);
  const decompress = opts.decompress;
  const isFile = wasmUrl.protocol === "file:";

  // make file urls work in Node via dnt
  const isNode = globalThis.process?.versions?.node != null;
  if (isNode && isFile) {
    // the deno global will be shimmed by dnt
    const wasmCode = await Deno.readFile(wasmUrl);
    return WebAssembly.instantiate(
      decompress ? decompress(wasmCode) : wasmCode,
      imports,
    );
  }

  switch (wasmUrl.protocol) {
    case "file:":
    case "https:":
    case "http:": {
      if (isFile) {
        if (typeof Deno !== "object") {
          throw new Error("file urls are not supported in this environment");
        }
        if ("permissions" in Deno) {
          await Deno.permissions.request({ name: "read", path: wasmUrl });
        }
      } else if (typeof Deno === "object" && "permissions" in Deno) {
        await Deno.permissions.request({ name: "net", host: wasmUrl.host });
      }
      const wasmResponse = await fetch(wasmUrl);
      if (decompress) {
        const wasmCode = new Uint8Array(await wasmResponse.arrayBuffer());
        return WebAssembly.instantiate(decompress(wasmCode), imports);
      }
      if (
        isFile ||
        wasmResponse.headers.get("content-type")?.toLowerCase()
          .startsWith("application/wasm")
      ) {
        return WebAssembly.instantiateStreaming(wasmResponse, imports);
      } else {
        return WebAssembly.instantiate(
          await wasmResponse.arrayBuffer(),
          imports,
        );
      }
    }
    default:
      throw new Error(`Unsupported protocol: ${wasmUrl.protocol}`);
  }
}
