# plan — open items from CL10/CL11 session.

three features left from the brainfuck/match session.
ordered by dependency: `as` cast unblocks brainfuck `.`,
string match unblocks match on runtime strings, realloc
invalidation hardens dynamic arrays.

---

## phase 1 — `as` cast (int to char, int to float, etc.).

`expr as Type` — type conversion. immediate use case:
`tape[ptr] as char` for brainfuck `.` command.

### design.

new SIR instruction:
```
Insn::Cast { dst: ValueId, src: ValueId, from_ty: TyId, to_ty: TyId }
```

executor: `Token::As` handler — pop value from stacks, read
target type keyword, emit Cast. same pattern as unary ops.

codegen dispatch on (from, to):
- `int -> char` — no-op (UXTB, truncate to byte, same GP reg)
- `char -> int` — no-op (already zero-extended from LDRB)
- `int -> float` — SCVTF Dn, Xn
- `float -> int` — FCVTZS Xn, Dn
- `int -> bytes` — no-op (same as char)
- `bytes -> int` — no-op

register allocator: Cast = copy semantics. input live until
output produced. float<->int needs GP<->FP register transfer.

### steps.

1. add `Token::As` to zo-token (check if exists)
2. add `Insn::Cast` to zo-sir
3. executor: handle `Token::As` — pop value, resolve type, emit
4. codegen: dispatch (from, to) pairs
5. register allocator: handle Cast (insn_is_fp, allocation)
6. test: `as-cast.zo` — all conversion pairs
7. verify: `brainfuck.zo` can use `.` command with `as char`

### traps.

- `as` keyword conflict? check grammar/token list
- float<->int needs GP<->FP register move (FMOV or SCVTF/FCVTZS)
- `insn_is_fp` must return true when `to_ty` is float

---

## phase 2 — string match (content equality).

`match s { "foo" => ... }` — currently pointer comparison.
two strings with same content but different addresses won't
match if the scrutinee is runtime-constructed.

### design.

**step 1: check intern path.** all string literals are
interned — same content = same Symbol = same pointer. match
patterns are always literals. if the scrutinee is also a
literal (or loaded from a Store of a literal), the interned
pointer already matches. test this first.

**step 2: full solution.** when `BinOp::Eq` has `ty_id = str`,
emit a `_strcmp` call via the extern stub infrastructure (same
as `_malloc`/`_realloc`). compare result to 0.

codegen for `BinOp::Eq` on str:
```
; load char* from string struct (offset 8)
LDR X0, [Xarray, #8]    ; lhs string data pointer
LDR X1, [Xother, #8]    ; rhs string data pointer
BL _strcmp               ; returns 0 if equal
CMP X0, #0
CSET Xdst, EQ
```

### steps.

1. test: does `match s { "hello" => ... }` work when s is a
   literal assigned via `imu s: str = "hello"`?
2. if yes: document limitation, add test
3. add `_strcmp` to extern stubs (same pattern as `_malloc`)
4. codegen: `BinOp::Eq` with str ty_id → emit strcmp call
5. caller-save register spill around strcmp
6. test: `match-string.zo` — literal and runtime strings

### traps.

- string struct layout: [len:8][ptr:8] — data pointer at
  offset 8, not offset 0
- caller-save registers must be saved around strcmp BL
- null-terminated? zo strings may not be null-terminated —
  may need `_memcmp` with length instead of `_strcmp`

---

## phase 3 — realloc pointer invalidation.

after `_realloc`, the old pointer is invalid. the register
allocator may still hold the old pointer in a register.
currently mitigated by `initial_cap=1024`.

### design.

in the register allocator, after `ArrayPush`, mark the array
operand's ValueId register as expired. any subsequent `Load`
of the same array re-reads from the stack slot (which has the
updated pointer from the writeback).

### steps.

1. register allocator: after processing `ArrayPush`, find the
   register holding `array` ValueId, mark it as dead/expired
2. subsequent `Load` of same symbol re-allocates a register
   and re-reads from stack
3. test: program that pushes > 1024 elements, reads after push
4. remove initial_cap=1024 workaround (or keep as optimization)

### traps.

- identifying which register holds the array: ArrayPush has
  `array: ValueId` — look up in allocation map
- must not expire the register BEFORE the push instruction
  completes (the push itself needs the old pointer to read
  len/cap from the header)
- the writeback in codegen stores new pointer to mutable_slot
  AFTER realloc — the expire must happen after this point

---

## order of execution.

| phase | feature | difficulty | blocked by |
|-------|---------|------------|------------|
| 1     | `as` cast | medium | nothing |
| 2     | string match | small-medium | nothing |
| 3     | realloc invalidation | small | nothing |

all three are independent. phase 1 is most impactful (unblocks
brainfuck `.` command and general type conversion). phase 2 is
a correctness issue. phase 3 is a robustness hardening.
