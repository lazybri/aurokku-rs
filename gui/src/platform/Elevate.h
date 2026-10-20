#pragma once

/// Linux start-up privilege elevation via polkit / `pkexec`.
namespace Elevate {
/// True when the process is effectively root (always true off Linux).
bool isEffectiveRoot();

/// Re-execute with root privileges on Linux when possible. Returns normally
/// when the process should continue unprivileged; never returns when the
/// elevated child already ran.
void maybeElevate(int argc, char **argv);
} // namespace Elevate
