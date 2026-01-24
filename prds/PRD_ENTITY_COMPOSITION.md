# PRD: Entity Composition for Reduced Boilerplate

**Status:** Draft
**Author:** GitHub Copilot
**Date:** 2026-01-23

## Overview

Refactor binnacle's primary entity types (Task, Bug, Idea, Milestone) to use struct composition via a shared `EntityCore` struct. This eliminates duplicated field definitions and `Entity` trait implementations, reducing boilerplate from ~80 lines per new entity type to ~10 lines.

## Motivation

The current patch adding `short_name` to Bug/Idea/Milestone demonstrates the problem:

- **422 lines added** for a single optional field across 4 entity types
- Each entity duplicates 8 identical fields (`id`, `entity_type`, `title`, `short_name`, `description`, `tags`, `created_at`, `updated_at`)
- Each entity requires a 27-line `impl Entity` block with identical delegation logic
- Adding a new entity type requires copying ~80 lines of boilerplate
- Adding a new common field (like `short_name`) requires touching every entity

With more node types planned, this maintenance burden will compound.

## Non-Goals

- **Proc-macro derive** — A `#[derive(Entity)]` macro would reduce boilerplate further but adds complexity. Out of scope unless we exceed 8+ entity types.
- **Breaking JSON format** — Existing serialized data must deserialize without migration.
- **Changing public API semantics** — Field access patterns may change syntactically but not semantically.

## Dependencies

- None — this is a pure refactor with no feature dependencies.

---

## Specification

### EntityCore Struct

A new struct containing all fields common to primary entities:

```rust
/// Common fields shared by all primary entity types.
///
/// Use `#[serde(flatten)]` when embedding in entity structs to maintain
/// flat JSON serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityCore {
    /// Unique identifier (e.g., "bn-a1b2")
    pub id: String,

    /// Entity type marker (e.g., "task", "bug", "idea", "milestone")
    #[serde(rename = "type")]
    pub entity_type: String,

    /// Entity title
    pub title: String,

    /// Optional short display name (shown in GUI instead of ID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,

    /// Detailed description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}
```

### EntityCore Constructor

```rust
impl EntityCore {
    /// Create a new EntityCore with the given type, ID, and title.
    pub fn new(entity_type: &str, id: String, title: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            entity_type: entity_type.to_string(),
            title,
            short_name: None,
            description: None,
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}
```

### Entity Trait Implementation

Implement `Entity` directly on `EntityCore`:

```rust
impl Entity for EntityCore {
    fn id(&self) -> &str { &self.id }
    fn entity_type(&self) -> &str { &self.entity_type }
    fn title(&self) -> &str { &self.title }
    fn short_name(&self) -> Option<&str> { self.short_name.as_deref() }
    fn description(&self) -> Option<&str> { self.description.as_deref() }
    fn created_at(&self) -> DateTime<Utc> { self.created_at }
    fn updated_at(&self) -> DateTime<Utc> { self.updated_at }
    fn tags(&self) -> &[String] { &self.tags }
}
```

### Refactored Entity Structs

Each entity embeds `EntityCore` with `#[serde(flatten)]`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bug {
    /// Common entity fields (id, title, short_name, etc.)
    #[serde(flatten)]
    pub core: EntityCore,

    // Bug-specific fields only:
    /// Priority level (0-4, lower is higher priority)
    #[serde(default)]
    pub priority: u8,

    /// Current status
    #[serde(default)]
    pub status: TaskStatus,

    /// Severity level
    #[serde(default)]
    pub severity: BugSeverity,

    /// Steps to reproduce
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reproduction_steps: Option<String>,

    // ... remaining bug-specific fields
}
```

### Entity Trait Delegation

Each entity type delegates to its `core` field. Two options:

**Option A: Explicit delegation (recommended)**

```rust
impl Entity for Bug {
    fn id(&self) -> &str { self.core.id() }
    fn entity_type(&self) -> &str { self.core.entity_type() }
    fn title(&self) -> &str { self.core.title() }
    fn short_name(&self) -> Option<&str> { self.core.short_name() }
    fn description(&self) -> Option<&str> { self.core.description() }
    fn created_at(&self) -> DateTime<Utc> { self.core.created_at() }
    fn updated_at(&self) -> DateTime<Utc> { self.core.updated_at() }
    fn tags(&self) -> &[String] { self.core.tags() }
}
```

**Option B: Blanket impl via AsRef (advanced)**

```rust
pub trait HasEntityCore {
    fn core(&self) -> &EntityCore;
}

impl<T: HasEntityCore> Entity for T {
    fn id(&self) -> &str { &self.core().id }
    // ... delegate all methods
}

impl HasEntityCore for Bug {
    fn core(&self) -> &EntityCore { &self.core }
}
```

Option A is preferred for clarity and debuggability.

### Simplified Constructors

```rust
impl Bug {
    pub fn new(id: String, title: String) -> Self {
        Self {
            core: EntityCore::new("bug", id, title),
            priority: 2,
            status: TaskStatus::default(),
            severity: BugSeverity::default(),
            reproduction_steps: None,
            affected_component: None,
            assignee: None,
            depends_on: Vec::new(),
            closed_at: None,
            closed_reason: None,
        }
    }
}
```

### Field Access Patterns

After refactoring, field access changes:

| Before | After |
|--------|-------|
| `bug.id` | `bug.core.id` |
| `bug.title` | `bug.core.title` |
| `bug.short_name` | `bug.core.short_name` |
| `bug.severity` | `bug.severity` (unchanged) |

Alternatively, add convenience accessors to maintain ergonomics:

```rust
impl Bug {
    pub fn id(&self) -> &str { &self.core.id }
    pub fn title(&self) -> &str { &self.core.title }
    // etc.
}
```

Or use `Deref` (controversial but eliminates `.core`):

```rust
impl std::ops::Deref for Bug {
    type Target = EntityCore;
    fn deref(&self) -> &Self::Target { &self.core }
}
// Now `bug.id` works directly
```

**Recommendation:** Use explicit `.core` access. It's clear, grep-able, and avoids `Deref` anti-pattern concerns.

### JSON Compatibility

`#[serde(flatten)]` ensures JSON format is unchanged:

```json
{
  "id": "bn-a1b2",
  "type": "bug",
  "title": "Login fails",
  "short_name": "login-bug",
  "description": "...",
  "tags": ["auth"],
  "created_at": "2026-01-23T00:00:00Z",
  "updated_at": "2026-01-23T00:00:00Z",
  "severity": "high",
  "priority": 1
}
```

Fields serialize flat, not nested under `"core"`.

---

## Implementation

### Files to Modify

| File | Changes |
|------|---------|
| [src/models/mod.rs](src/models/mod.rs) | Add `EntityCore`, refactor Task/Bug/Idea/Milestone, update `impl Entity` |
| [src/commands/mod.rs](src/commands/mod.rs) | Update field access from `entity.field` to `entity.core.field` |
| [src/main.rs](src/main.rs) | Update any direct field access |
| [src/mcp/mod.rs](src/mcp/mod.rs) | Update field access in MCP handlers |
| [src/gui/mod.rs](src/gui/mod.rs) | Update field access in GUI serialization |
| Tests | Update field access patterns |

### Migration Strategy

1. **Phase 1:** Add `EntityCore` struct and implement `Entity` on it
2. **Phase 2:** Refactor one entity (Bug) as proof of concept
3. **Phase 3:** Run full test suite, verify JSON round-trip
4. **Phase 4:** Refactor remaining entities (Task, Idea, Milestone)
5. **Phase 5:** Update all field access sites
6. **Phase 6:** Remove duplicated field definitions

### Boilerplate Comparison

| Metric | Before | After |
|--------|--------|-------|
| Lines per new entity | ~80 | ~15 |
| Lines to add common field | ~20 per entity | ~2 (in EntityCore) |
| `impl Entity` per type | 27 lines | 10 lines (delegation) |

---

## Testing

1. **JSON round-trip test** — Serialize existing entities, deserialize with new structs, verify equality
2. **Backward compatibility test** — Load JSON from current `main` branch, verify it deserializes correctly
3. **Entity trait test** — Existing `test_all_primary_entities_implement_entity_trait` should pass unchanged
4. **CLI integration tests** — All existing CLI tests should pass
5. **GUI rendering test** — Verify GUI displays entities correctly after refactor

### Example Round-Trip Test

```rust
#[test]
fn test_bug_json_backward_compatibility() {
    // JSON from before the refactor
    let old_json = r#"{
        "id": "bn-test",
        "type": "bug",
        "title": "Test Bug",
        "short_name": "test",
        "description": "A test bug",
        "tags": ["test"],
        "created_at": "2026-01-23T00:00:00Z",
        "updated_at": "2026-01-23T00:00:00Z",
        "priority": 2,
        "status": "pending",
        "severity": "medium"
    }"#;

    let bug: Bug = serde_json::from_str(old_json).unwrap();
    assert_eq!(bug.core.id, "bn-test");
    assert_eq!(bug.core.title, "Test Bug");
    assert_eq!(bug.core.short_name, Some("test".to_string()));
    assert_eq!(bug.severity, BugSeverity::Medium);
}
```

---

## Design Decisions

1. **No Deref** — Entities will NOT implement `Deref<Target=EntityCore>`. While it enables `bug.title` syntax, it's controversial in Rust (hides indirection, confuses tooling, anti-pattern for non-smart-pointer types). Callers use explicit `bug.core.title` — it's clear, grep-able, and honest about the structure.

2. **Explicit delegation** — Each entity type gets its own `impl Entity` block that delegates to `self.core`. This is ~10 lines per type but easier to debug, step through, and understand than a blanket impl with trait magic.

3. **No accessor methods** — Entities won't have convenience methods like `fn title(&self) -> &str`. The `Entity` trait already provides these via `entity.id()`, `entity.title()`, etc. For direct field access, use `.core.field`. Adding accessors would duplicate the trait methods.

4. **Minimal EntityCore** — Only truly universal fields go in `EntityCore`: `id`, `entity_type`, `title`, `short_name`, `description`, `tags`, `created_at`, `updated_at`. Fields like `priority`, `status`, `assignee` stay entity-specific since Idea has different semantics (no priority, different status enum).

5. **No ClosureInfo substruct** — Keep `closed_at`/`closed_reason` as entity-specific fields. Only 3 of 4 current entities have them, and adding another layer of composition adds complexity without enough benefit. Revisit if we add 3+ more closeable entity types.

## Open Questions

None — all design questions resolved.
