# Engine-Owned Pointer Lifetimes and Table Safety

## Status

The table API has been redesigned to reduce the risk of dangling pointers and
aliased mutable references:

- The public persistent `Table<'a>` pointer wrapper has been removed.
- `TableId` is now the persistent identity of a function table.
- `read_table()` returns an owned snapshot.
- `table_copy_in()` and `table_copy_out()` perform synchronous copies through
  Csound's internally locked copy API.
- `with_table()` provides scoped zero-copy access and requires `&mut Csound`.
- Safe asynchronous table-copy operations are not exposed.
- Table-generation arguments are returned as owned data rather than a public
  borrowed slice.

This substantially reduces the table risks: safe Rust code can no longer keep a
function-table pointer alive across `perform_ksmps()`, recompilation, or a later
score event. It also prevents obtaining multiple persistent mutable table views.

The redesign does not make arbitrary concurrent access safe. A separate Csound
performance thread or already queued asynchronous compilation can still operate
independently of Rust's borrow checker. Those remaining constraints are
explained below.

## Table API sequencing

Scoped zero-copy access takes an exclusive borrow:

```rust
pub fn with_table<R>(
    &mut self,
    id: TableId,
    f: impl for<'table> FnOnce(&'table mut [Myflt]) -> R,
) -> Result<R>;
```

This sequences direct table access with `perform_ksmps()`, compilation, and
other safe calls made through the same `Csound` value. The table slice cannot
escape the closure.

Owned copy operations take `&self`:

```rust
pub fn read_table(&self, id: TableId) -> Result<Vec<Myflt>>;
pub fn table_copy_out(&self, id: TableId, dest: &mut [Myflt]) -> Result<usize>;
pub fn table_copy_in(&self, id: TableId, src: &[Myflt]) -> Result<usize>;
```

These methods do not return references into Csound. They always use the
synchronous C API variant (`async = 0`), and Csound holds its API lock while the
copy is performed. This synchronized interior mutation is why copy operations
can use `&self`.

## Engine-owned pointers during performance

The following table excludes `reset()`, which invalidates essentially all
engine-owned allocations.

| Pointer or storage | Can its address change during score performance? | Can its contents change? | Csound protection |
|---|---|---|---|
| Function-table data (`FUNC::ftable`) | **Yes.** It can be relocated when resized or replaced with a different size, and freed when deleted, by `ftfree`, or during temporary-table teardown. | **Yes**, even without relocation. | No dedicated table-view lock. `csoundGetTable()` and its returned pointer are explicitly non-thread-safe. |
| Function-table arguments (`FUNC::args`) | **Yes.** The allocation is generally replaced on every table redefinition, including same-size replacement. | **Yes.** | Same as table data. `csoundGetTableArgs()` and its returned pointer are explicitly non-thread-safe. |
| `spin` input buffer | Normally no; it is allocated when the engine starts. | **Yes**, during each processing cycle or input operation. | No lock is associated with `csoundGetSpin()`. Host access is sequenced before `perform_ksmps()`. |
| `spout` output buffer | Normally no; it is allocated when the engine starts. | **Yes**, during each `perform_ksmps()`. | No lock is associated with `csoundGetSpout()`. Host access is sequenced after `perform_ksmps()`. |
| Control-channel storage | Normally no after creation. | **Yes.** | Control access uses atomics where available and otherwise the per-channel lock. Raw-pointer access is not automatically locked. |
| Audio-channel storage | Normally no after creation because global `ksmps` is fixed. | **Yes**, during performance. | A per-channel lock is available and should be used by host access. Raw `csoundGetChannelPtr()` access is not automatically locked. |
| String-channel `STRINGDAT` structure | Normally no after channel creation. | **Yes.** | The structure remains in the channel database until reset. |
| `STRINGDAT::data` | **Yes.** String writes reallocate the buffer when its capacity is exceeded. | **Yes.** | Intended to be protected by the per-channel lock. `csoundSetStringData()` does not acquire the lock itself; its caller must do so. |
| PVS-channel `PVSDAT` structure | Normally no after channel creation. | **Yes.** | Per-channel lock. |
| `PVSDAT::frame.auxp` | **Yes**, during initial allocation or when an instrument initializes a larger frame. | **Yes.** | PVS channel opcodes perform frame allocation and copying under the per-channel lock. |
| Array-channel data | **Yes**, if its shape or capacity changes. | **Yes.** | Per-channel locking plus internal array-storage locking. Raw array-data pointers should not persist. |
| Host-created circular-buffer allocation | Not as a result of score execution. | **Yes.** | Circular-buffer synchronization rules apply. Score execution does not normally relocate the allocation. |
| Callback audio or MIDI buffers | A callback may receive a different pointer on every invocation. | **Yes.** | Valid only for the callback invocation; the callback lifetime is the protection. |

## Function tables are exceptional

An ordinary score can invalidate a function-table pointer without any host API
call:

```csound
f 1 0.05 1024 2 2
```

When that event is processed, table 1 may be replaced while
`perform_ksmps()` is executing.

Instrument initialization can produce the same effect through:

- `ftgen`
- `ftgentmp`
- `ftfree`
- `ftload`
- Other opcodes using `FTAlloc`, `FTCreate`, or `FTDelete`

Table-writing opcodes can also change table contents without relocating the
allocation. Consequently, even an immutable Rust slice into a table cannot
safely coexist with score performance.

The old persistent `Table<'a>` abstraction tied the pointer lifetime to the
`Csound` instance, but Csound's actual guarantee was shorter: the pointer was
valid only until Csound replaced or deleted that particular table. The new API
models that distinction by keeping only `TableId` persistent.

## What Csound's locks provide

Csound contains several synchronization mechanisms:

- `API_lock`
- `init_pass_threadlock`
- Realtime allocation locks
- Per-channel locks

None of them provide a public operation that pins a function table and keeps its
address valid for an arbitrary host-controlled scope.

`csoundTableCopyIn()` and `csoundTableCopyOut()` synchronously acquire
`API_lock`. Their internal implementations also use `init_pass_threadlock` in
realtime mode. They are therefore preferable to copying through a raw table
pointer.

There are still important limitations:

- `csoundGetTable()` does not acquire these locks.
- No Csound lock is held across a Rust `with_table()` closure.
- Score and opcode execution can resize or delete tables.
- Already queued asynchronous compilation can mutate tables independently of
  Rust's `&mut Csound`.
- The Rust copy wrappers query the table length before the C copy acquires
  `API_lock`. Concurrent resizing can make that capacity check stale.

Therefore, `&mut Csound` provides Rust-side sequencing; it is not itself a
Csound table lock.

## Current contracts

### Owned reads and writes

`read_table()`, `table_copy_out()`, and `table_copy_in()`:

1. Use only synchronous Csound table copies (`async = 0`).
2. Do not expose references into engine memory.
3. May use `&self` because Csound synchronizes the copy internally.
4. May block performance or other API operations while copying a large table.
5. Require that another thread or pending asynchronous compilation does not
   resize or delete the table between the length query and the copy.

The asynchronous C variants are not exposed safely because they queue a raw
pointer and return immediately. Rust could otherwise read, modify, or drop the
source or destination allocation before Csound consumed it.

### Scoped zero-copy access

`with_table()`:

1. Requires `&mut Csound`.
2. Prevents other safe Rust calls on the same instance while the slice exists.
3. Prevents the slice from escaping its closure.
4. Must only be used while no separate performance thread is running.
5. Must not be used while asynchronous compilation capable of replacing the
   table is pending.
6. Exposes only the logical table data; the Csound guard point is not included.

A normal synchronous performance loop can alternate direct table access and
performance safely:

```rust
loop {
    cs.with_table(table_id, |table| {
        // Direct access while Csound is not executing.
        table[0] *= 0.5;
    })?;

    if cs.perform_ksmps() {
        break;
    }
}
```

The exclusive table borrow ends when the closure returns, so the following
`perform_ksmps()` call is allowed.

## Remaining work and stronger guarantees

For a separately running performance thread or pending asynchronous
compilation:

- `with_table()` must be unavailable or treated as unsafe.
- Owned operations should continue to use Csound's synchronous copy API.
- Dynamically sized copies still need protection against a table resize between
  the size query and the locked copy.

A fully concurrent, dynamically sized snapshot would ideally use a new Csound C
API that determines the current table length and copies the data while holding
one lock for the complete operation.

Until such an API exists, the current design intentionally favors:

- Persistent logical identifiers (`TableId`)
- Owned snapshots for ordinary reads
- Synchronous internally locked copies
- Short, explicitly exclusive zero-copy scopes

This removes the principal safe-code use-after-free paths while keeping the
remaining C-level concurrency limitations explicit.
