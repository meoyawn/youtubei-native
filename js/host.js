/* The UTF-16 transport needs no JavaScript web APIs during startup. */
(function () {
  const begin = $SHBuiltin.extern_c({},
    function youtubei_host_begin(length: c_int): void {});
  const write = $SHBuiltin.extern_c({},
    function youtubei_host_write(index: c_int, unit: c_int): void {});
  const call = $SHBuiltin.extern_c({},
    function youtubei_host_call(): c_int { throw 0; });
  const data = $SHBuiltin.extern_c({},
    function youtubei_host_data(): c_ptr { throw 0; });
  const copyUtf8 = $SHBuiltin.extern_c({declared: true, hv: true},
    function _sh_asciiz_to_string(runtime: c_ptr, data: c_ptr, length: c_ptrdiff_t): string { throw 0; });
  const globals: any = globalThis;
  globals.nativeTransport = function (request: string): string {
    begin(request.length);
    for (let i = 0; i < request.length; i++) write(i, request.charCodeAt(i));
    const length = call();
    return copyUtf8($SHBuiltin.c_native_runtime(), data(), length);
  };
})();
