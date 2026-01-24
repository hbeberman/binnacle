# Idea: KDL Plugin Specification Format

**Status:** Early exploration  
**Created:** 2026-01-24

## Problem Statement

Binnacle's entity types (tasks, bugs, ideas, etc.) are hardcoded. Users may want to:
- Add custom entity types (tickets, epics, sprints)
- Add custom fields to existing types
- Define custom edge types with constraints
- React to graph events with automated actions

## Proposed Solution

Use **KDL** (the document language) as a declarative specification format for extending the graph schema and defining event-driven behaviors.

### Layer 1: Schema DSL

Define custom entities, fields, and edges:

```kdl
entity "ticket" {
  inherit "task"
  field "severity" type="enum" values="low,medium,high,critical"
  field "customer" type="string" optional=true
  field "sla_hours" type="int" default=24
}

entity "sprint" {
  field "start_date" type="date" required=true
  field "end_date" type="date" required=true
  field "velocity" type="int"
}

edge "escalates_to" {
  from "ticket"
  to "ticket"
  field "reason" type="string"
}

edge "assigned_to_sprint" {
  from "task" "ticket" "bug"
  to "sprint"
  cardinality "many-to-one"
}
```

### Layer 2: Event/Action DSL

React to graph mutations:

```kdl
on "ticket.created" {
  when severity="critical" {
    action "notify" channel="slack" message="Critical ticket: {title}"
  }
  when severity="high" sla_hours<4 {
    action "add_tag" tag="urgent"
  }
}

on "ticket.status_changed" {
  when status="closed" previous="in_progress" {
    action "create_edge" type="resolved_by" to="{current_agent}"
  }
}

on "task.blocked" {
  action "notify" channel="stdout" message="Task {id} blocked: {blocked_reason}"
}

on "sprint.end_date_reached" {
  action "report" type="velocity" include_incomplete=true
}
```

## Implementation Options for Event Actions

| Approach | Power | Safety | Complexity |
|----------|-------|--------|------------|
| **Declarative rules** (above) | Limited | High | Low |
| **Starlark scripts** | Medium | Medium (sandboxed) | Medium |
| **Lua embedding** | High | Medium | Medium |
| **WASM plugins** | Full | High (sandboxed) | High |

### Recommendation

Start with **declarative rules** for common patterns:
- `notify` - Send message to channel (slack, stdout, webhook)
- `create_edge` - Add relationship
- `add_tag` / `remove_tag` - Modify tags
- `set_field` - Update field value
- `create_entity` - Spawn related entity

Graduate to **WASM plugins** for complex logic if needed.

## Schema Validation

```kdl
// Constraints on the graph structure
constraint "no_circular_deps" {
  edge "depends_on"
  rule "acyclic"
}

constraint "sprint_capacity" {
  entity "sprint"
  rule "max_children" edge="assigned_to_sprint" limit=20
}

constraint "critical_needs_assignee" {
  entity "ticket"
  when severity="critical"
  rule "required_edge" type="assigned_to"
}
```

## Open Questions

1. **Hot reload**: Can schema changes apply without restart?
2. **Migration**: How to handle schema changes with existing data?
3. **Inheritance**: How deep can entity inheritance go? Multiple inheritance?
4. **Validation timing**: Validate on write or allow invalid states temporarily?
5. **Plugin distribution**: How are plugins shared? (git submodule, registry, inline)

## Related Ideas

- `bn-483a` - Distributed Subgraph Coordination (plugins may define subgraph boundaries)

## Why KDL?

- Human-readable and writable
- Supports nested structures naturally
- Less noisy than JSON/YAML for config
- Rust crate available (`kdl`)
- Already used by tools like `zellij`

## Next Steps

- [ ] Prototype schema DSL parser
- [ ] Define minimal set of built-in actions
- [ ] Explore WASM plugin interface for escape hatch
- [ ] Consider how this interacts with storage layer (custom fields in JSONL?)
