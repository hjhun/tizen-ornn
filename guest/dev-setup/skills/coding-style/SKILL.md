---
name: coding-style
description: Apply C and C++ conventions for native services, public C facades, callbacks, build definitions and RPM packaging.
---

# Coding style

- Follow the repository's language standard and public naming conventions. Keep new C++ within 80 columns, use two spaces, attached braces, and separate system and project includes. Include the matching header first and include the declarations you use.
- Give each class one identifiable responsibility. Keep public C handles and entry points thin. Delegate ownership, dispatch and cleanup to the implementing object. Use explicit ownership, RAII with the matching deleter, and deliberate copy and move semantics.
- Use PascalCase for C++ types and methods, descriptive snake_case locals, trailing underscores for data members, and k-prefixed constants. Preserve exported C symbols, generated names and wire identifiers.
- Keep callbacks short. Indicate whether references are borrowed or owned, which thread or main context runs the callback, and when cancellation prevents further calls. Keep cleanup safe during callback reentry.
- Prefer direct platform APIs and small interfaces at real dependency boundaries. Avoid adding production switches solely for tests. Separate state changes from result construction when order affects correctness.
- Group includes by purpose. Preserve initialization order where lifetime matters. Avoid broad namespace imports, global state, and logging secrets or whole payloads.
- Change generator inputs and regenerate their outputs. Keep CMake targets and pkg-config dependencies scoped to the component using them. Keep runtime, development and test RPM contents distinct.
