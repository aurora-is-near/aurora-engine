#[cfg(test)]
mod benches;
#[cfg(test)]
mod prelude;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod utils;

// `near-vm-vm 0.35` declares `extern "C" fn __rust_probestack()`; rustc 1.87+ no
// longer exports that symbol with C linkage from `compiler-builtins`, so the
// native test binary fails to link with `undefined symbol: __rust_probestack`.
// A bare `ret` satisfies the linker (stack-probe guard-page touching is disabled
// for the JITed wasm the harness runs in-process, which is fine for tests only).
#[cfg(all(test, target_arch = "x86_64", target_os = "linux"))]
core::arch::global_asm!(".globl __rust_probestack", "__rust_probestack:", "    ret",);
