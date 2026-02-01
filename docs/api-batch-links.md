# Batch Link Creation API

## Endpoint

`POST /api/links/batch`

## Description

Creates multiple links (edges) between entities in a single request. This is useful for batch operations like setting up multiple dependencies or relationships at once.

## Request Body

```json
{
  "links": [
    {
      "source": "bn-xxxx",
      "target": "bn-yyyy",
      "edge_type": "depends_on"
    },
    {
      "source": "bn-yyyy",
      "target": "bn-zzzz",
      "edge_type": "child_of"
    }
  ]
}
```

### Fields

- `links`: Array of link objects to create
  - `source`: ID of the source entity
  - `target`: ID of the target entity  
  - `edge_type`: Type of relationship (e.g., "depends_on", "child_of", "queued", "blocks", etc.)

## Response

### Success (all links created)

```json
{
  "success": true,
  "total": 2,
  "success_count": 2,
  "error_count": 0,
  "results": [
    {
      "success": true,
      "edge": {
        "id": "bne-abc123",
        "source": "bn-xxxx",
        "target": "bn-yyyy",
        "edge_type": "depends_on"
      }
    },
    {
      "success": true,
      "edge": {
        "id": "bne-def456",
        "source": "bn-yyyy",
        "target": "bn-zzzz",
        "edge_type": "child_of"
      }
    }
  ]
}
```

### Partial Failure (some links failed)

```json
{
  "success": false,
  "total": 2,
  "success_count": 1,
  "error_count": 1,
  "results": [
    {
      "success": true,
      "edge": {
        "id": "bne-abc123",
        "source": "bn-xxxx",
        "target": "bn-yyyy",
        "edge_type": "depends_on"
      }
    },
    {
      "success": false,
      "error": "Invalid edge type: invalid_type"
    }
  ]
}
```

## Error Handling

- Individual link failures do not stop processing of other links
- Each result includes either an `edge` object (success) or an `error` string (failure)
- Overall `success` is `true` only if all links were created successfully
- Common errors:
  - Invalid edge type
  - Duplicate link (source + target + type already exists)
  - Non-existent source or target entity
  - Cycle detection (for dependency relationships)

## Readonly Mode

In readonly mode, this endpoint returns HTTP 403 with:

```json
{
  "error": "Server is in readonly mode - write operations are disabled"
}
```

## Example Usage

### curl

```bash
curl -X POST \
  -H "Content-Type: application/json" \
  -d '{
    "links": [
      {"source": "bn-1234", "target": "bn-5678", "edge_type": "depends_on"},
      {"source": "bn-5678", "target": "bn-9abc", "edge_type": "depends_on"}
    ]
  }' \
  http://localhost:3030/api/links/batch
```

### JavaScript (fetch)

```javascript
const response = await fetch('http://localhost:3030/api/links/batch', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    links: [
      { source: 'bn-1234', target: 'bn-5678', edge_type: 'depends_on' },
      { source: 'bn-5678', target: 'bn-9abc', edge_type: 'depends_on' }
    ]
  })
});

const result = await response.json();
console.log(`Created ${result.success_count} links, ${result.error_count} errors`);
```

## Use Cases

- Creating a chain of dependencies between tasks
- Adding multiple tasks to a queue at once
- Linking multiple tasks to a milestone
- Setting up complex task relationships in one operation
