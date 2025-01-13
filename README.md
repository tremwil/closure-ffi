# closure_ffi

Rust library providing wrappers around closures which allows them to be called through context-free 
unsafe bare functions.

Context-free bare functions are not needed very often, as properly designed C APIs typically
allow the user to specify an opaque pointer to a context object which will be provided to the
function pointer. However, this is not always the case, and may be impossible in less common
scenarios, e.g. function hooking for game modding/hacking.