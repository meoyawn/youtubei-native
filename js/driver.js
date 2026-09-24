/* Flow native types are consumed by shermes, not by esbuild. */
(function () {
  const inputLength = $SHBuiltin.extern_c({},
    function youtubei_input_length(method: c_int): c_int { throw 0; });
  const inputByte = $SHBuiltin.extern_c({},
    function youtubei_input_byte(method: c_int, index: c_int): c_int { throw 0; });
  const outputByte = $SHBuiltin.extern_c({},
    function youtubei_output_byte(index: c_int, byte: c_int): void {});
  const complete = $SHBuiltin.extern_c({},
    function youtubei_complete(status: c_int, length: c_int): void {});
  const globals: any = globalThis;

  function read(method: number): string {
    const bytes: any = new globals.Uint8Array(inputLength(method));
    for (let i = 0; i < bytes.length; i++) bytes[i] = inputByte(method, i);
    return new globals.TextDecoder().decode(bytes);
  }

  function finish(status: number, value: any): void {
    const bytes: any = new globals.TextEncoder().encode(String(value));
    for (let i = 0; i < bytes.length; i++) outputByte(i, bytes[i]);
    complete(status, bytes.length);
  }

  let settled = false;
  function resolve(value: any): void {
    finish(0, value);
    settled = true;
  }
  function reject(error: any): void {
    finish(1, error && error.stack || error);
    settled = true;
  }
  try {
    const result: any = globals.youtubeiNative[read(1)](read(0));
    if (typeof result === "string") {
      resolve(result);
    } else {
      result.then(resolve, reject);
      globals.HermesInternal.drainJobs();
      if (!settled) reject("Offline operation did not settle after draining jobs");
    }
  } catch (error) {
    reject(error);
  }
})();
