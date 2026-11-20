---
name: development-style
description: Guide native application-framework changes from API and IPC contracts through lifetime analysis, implementation and verification.
---

# Development style

- Trace the caller, implementation and consumer before changing behavior. Identify ownership, errors, permissions, persistence, public C ABI and IPC compatibility. Respect the project's toolchain and existing callers.
- Split changes by responsibility. Keep useful objects and reuse intact. Favor the smallest change that addresses the cause, and keep refactoring separate from externally visible behavior changes.
- For asynchronous operations, trace registration, dispatch, completion, cancellation, disconnect and destruction. Keep callback targets alive until delivery completes. Avoid holding locks across application callbacks. Verify thread and main-context affinity.
- Preserve failure information across boundaries. Distinguish a failed request from transport failure. Keep partially initialized state and retry paths recoverable.
- For public types and generated IPC, check both producers and consumers, old persisted data, optional fields, argument order, and error handling. Edit the source schema or generator template, then verify generated consumers compile.
- Build and exercise the changed entry point with realistic inputs and errors. Test observable behavior, including retries and shutdown. For performance work, identify the workload, baseline, units, device and sample count.
- Report host tests, GBS builds, installed packages and device or emulator results separately. Tie results to the revision and artifacts actually tested and identify outstanding checks.
