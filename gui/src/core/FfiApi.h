#pragma once

// C ABI exported by `crates/auroku-ffi` (linked statically into the GUI as
// libauroku_ffi.a or auroku_ffi.lib). Every function returning `char*` returns
// a JSON envelope and the caller must release it with `auroku_string_free`.
extern "C" {
using AurokuLogCallback = void (*)(const char *line, void *user_data);

char *auroku_ffi_version();
char *auroku_scan_devices(int vid, int pid);
char *auroku_run_task(const char *request_json, AurokuLogCallback callback, void *user_data);
void auroku_string_free(char *text);
}
