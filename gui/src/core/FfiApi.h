#pragma once

// C ABI exported by `crates/auroku-ffi` (linked statically into the GUI as
// libauroku_ffi.a or auroku_ffi.lib). Every function returning `char*` returns
// a JSON envelope and the caller must release it with `auroku_string_free`.
#include <cstdint>

extern "C" {
using AurokuLogCallback = void (*)(const char *line, void *user_data);

char *auroku_ffi_version();
char *auroku_scan_devices(int vid, int pid);
// Requests may include the selected Linux USB "bus" and "address" to pin the
// operation to the exact device node whose ACL was granted.
char *auroku_run_task(const char *request_json, AurokuLogCallback callback, void *user_data);

// WinUSB driver provisioning (ABI 2). All of them return the usual JSON
// envelope; failures use kind = "DriverError".
char *auroku_driver_status();
char *auroku_driver_install(std::uint16_t vid, std::uint16_t pid, bool restore_on_exit,
                            AurokuLogCallback callback, void *user_data);
char *auroku_driver_restore(std::uint16_t vid, std::uint16_t pid, AurokuLogCallback callback,
                            void *user_data);
char *auroku_state_dir();

void auroku_string_free(char *text);
}
