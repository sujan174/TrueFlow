# Prompt Management

TrueFlow includes a built-in prompt registry for managing, versioning, and deploying prompt templates. This enables centralized prompt engineering without modifying agent code.

---

## Overview

**Key Features:**

- Versioned prompt templates with immutable history
- Folder organization for categorization
- Variable templating with server-side rendering
- Label-based deployment (`production`, `staging`, `development`)
- OpenAI-compatible output format

**Use Cases:**

- Centralize prompts across multiple agents
- Version control prompt iterations
- A/B test prompt variations
- Deploy prompt changes without code changes

---

## Quick Start

### 1. Create a Prompt

```bash
curl -X POST http://localhost:8443/api/v1/prompts \
  -H "Authorization: Bearer $ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Customer Support Agent",
    "slug": "customer-support-agent",
    "folder": "/support",
    "description": "Handles customer support inquiries"
  }'
```

### 2. Publish a Version

```bash
curl -X POST "http://localhost:8443/api/v1/prompts/{prompt_id}/versions" \
  -H "Authorization: Bearer $ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o",
    "messages": [
      {
        "role": "system",
        "content": "You are a customer support agent helping {{user_name}} with their account. Be helpful and concise."
      },
      {
        "role": "user",
        "content": "{{question}}"
      }
    ],
    "temperature": 0.7,
    "commit_message": "Initial version"
  }'
```

### 3. Deploy to Production

```bash
curl -X POST "http://localhost:8443/api/v1/prompts/{prompt_id}/deploy" \
  -H "Authorization: Bearer $ADMIN_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "version": 1,
    "label": "production"
  }'
```

### 4. Render at Runtime

```bash
curl "http://localhost:8443/api/v1/prompts/by-slug/customer-support-agent/render?label=production&user_name=Alice&question=Where%20is%20my%20order%3F" \
  -H "Authorization: Bearer $API_KEY"
```

Response:

```json
{
  "model": "gpt-4o",
  "messages": [
    {
      "role": "system",
      "content": "You are a customer support agent helping Alice with their account. Be helpful and concise."
    },
    {
      "role": "user",
      "content": "Where is my order?"
    }
  ],
  "temperature": 0.7,
  "prompt_id": "uuid",
  "prompt_slug": "customer-support-agent",
  "version": 1,
  "label": "production"
}
```

---

## API Reference

### List Prompts

`GET /prompts?folder={path}`

Returns all prompts, optionally filtered by folder.

```json
[
  {
    "id": "uuid",
    "name": "Customer Support Agent",
    "slug": "customer-support-agent",
    "folder": "/support",
    "description": "Handles customer support inquiries",
    "current_version": 2,
    "labels": ["production"],
    "created_at": "2026-03-15T10:00:00Z",
    "updated_at": "2026-03-16T14:30:00Z"
  }
]
```

### Create Prompt

`POST /prompts`

```json
{
  "name": "Prompt Name",
  "slug": "prompt-slug",
  "folder": "/category",
  "description": "Description"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Display name |
| `slug` | string | Yes | URL-friendly identifier (unique) |
| `folder` | string | No | Folder path for organization |
| `description` | string | No | Prompt description |

### Get Prompt

`GET /prompts/{id}`

Returns prompt with its latest version and all labels.

### Update Prompt Metadata

`PUT /prompts/{id}`

```json
{
  "name": "Updated Name",
  "folder": "/new-folder",
  "description": "Updated description"
}
```

### Delete Prompt

`DELETE /prompts/{id}`

Soft-deletes the prompt. Versions are retained for audit purposes.

---

## Version Management

### List Versions

`GET /prompts/{id}/versions`

Returns all versions with metadata:

```json
[
  {
    "version": 2,
    "model": "gpt-4o",
    "temperature": 0.7,
    "commit_message": "Improved tone",
    "created_at": "2026-03-16T14:30:00Z",
    "labels": ["production"]
  },
  {
    "version": 1,
    "model": "gpt-4o",
    "temperature": 0.7,
    "commit_message": "Initial version",
    "created_at": "2026-03-15T10:00:00Z",
    "labels": []
  }
]
```

### Publish New Version

`POST /prompts/{id}/versions`

```json
{
  "model": "gpt-4o",
  "messages": [
    { "role": "system", "content": "..." },
    { "role": "user", "content": "..." }
  ],
  "temperature": 0.7,
  "max_tokens": 1000,
  "commit_message": "Description of changes"
}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `model` | string | Yes | Default model for this prompt |
| `messages` | array | Yes | OpenAI-format message array |
| `temperature` | float | No | Sampling temperature |
| `max_tokens` | integer | No | Maximum output tokens |
| `commit_message` | string | No | Description of changes |

### Get Specific Version

`GET /prompts/{id}/versions/{version}`

Returns the full version content including messages.

---

## Deployment

### Labels

Labels are named pointers to specific versions, enabling environment-based deployment.

**Common Labels:**

| Label | Purpose |
|-------|---------|
| `production` | Live traffic |
| `staging` | Pre-production testing |
| `development` | Active development |

### Deploy Version

`POST /prompts/{id}/deploy`

```json
{
  "version": 2,
  "label": "production"
}
```

**Behavior:**

1. Version 2 is now tagged with `production`
2. Previous production version loses the label
3. Next render with `label=production` returns version 2
4. Atomic operation - no partial state

### Rollback

To rollback, deploy a previous version:

```bash
# Rollback to version 1
curl -X POST "http://localhost:8443/api/v1/prompts/{id}/deploy" \
  -d '{"version": 1, "label": "production"}'
```

---

## Variable Templating

### Syntax

Use `{{variable_name}}` syntax for variables:

```json
{
  "messages": [
    {
      "role": "system",
      "content": "You are helping {{user_name}} from {{company_name}}."
    },
    {
      "role": "user",
      "content": "{{question}}"
    }
  ]
}
```

### Rendering

Variables are substituted at render time:

**GET Request:**

```
GET /prompts/by-slug/{slug}/render?label=production&user_name=Alice&company_name=Acme&question=Help%20me
```

**POST Request:**

```
POST /prompts/by-slug/{slug}/render
{
  "label": "production",
  "variables": {
    "user_name": "Alice",
    "company_name": "Acme",
    "question": "Help me"
  }
}
```

### Missing Variables

If a variable is not provided:

- Default: Leave as `{{variable_name}}` (passthrough)
- Agent can provide at runtime
- Log warning for missing variables

---

## Rendering

### Render Endpoint (GET)

`GET /prompts/by-slug/{slug}/render?label={label}&{var1}={val1}&{var2}={val2}`

Query parameters are used as variables. Useful for simple cases.

### Render Endpoint (POST)

`POST /prompts/by-slug/{slug}/render`

```json
{
  "label": "production",
  "variables": {
    "user_name": "Alice",
    "question": "What is my balance?"
  }
}
```

POST is preferred for complex variables or special characters.

### Output Format

The render endpoint returns an OpenAI-compatible payload:

```json
{
  "model": "gpt-4o",
  "messages": [
    { "role": "system", "content": "..." },
    { "role": "user", "content": "..." }
  ],
  "temperature": 0.7,
  "max_tokens": 1000,
  "prompt_id": "uuid",
  "prompt_slug": "customer-support-agent",
  "version": 2,
  "label": "production"
}
```

This can be passed directly to the TrueFlow proxy:

```bash
# Render and proxy in one flow
PROMPT=$(curl -s "http://localhost:8443/api/v1/prompts/by-slug/my-prompt/render?label=production&var=val" \
  -H "Authorization: Bearer $API_KEY")

curl -X POST http://localhost:8443/v1/chat/completions \
  -H "Authorization: Bearer tf_v1_..." \
  -H "Content-Type: application/json" \
  -d "$PROMPT"
```

---

## Folder Organization

### List Folders

`GET /prompts/folders`

Returns unique folder paths:

```json
["/support", "/sales", "/engineering/code-review"]
```

### Filter by Folder

`GET /prompts?folder=/support`

Returns only prompts in the specified folder.

### Folder Structure

Use forward slashes for hierarchy:

```
/support
/support/escalation
/support/faq
/sales
/sales/outreach
/engineering
/engineering/code-review
/engineering/documentation
```

---

## Integration Patterns

### Agent Integration

Agents can fetch prompts at runtime:

```python
import requests

def get_rendered_prompt(slug, label, variables):
    response = requests.get(
        f"{TRUEFLOW_URL}/api/v1/prompts/by-slug/{slug}/render",
        headers={"Authorization": f"Bearer {API_KEY}"},
        params={"label": label, **variables}
    )
    return response.json()

# Get prompt
prompt = get_rendered_prompt(
    "customer-support-agent",
    "production",
    {"user_name": "Alice", "question": "Help me"}
)

# Use with LLM
response = requests.post(
    f"{TRUEFLOW_URL}/v1/chat/completions",
    headers={"Authorization": f"Bearer {TOKEN}"},
    json=prompt
)
```

### SDK Integration

```python
from trueflow import TrueFlowClient

client = TrueFlowClient(api_key="...", token="tf_v1_...")

# Render prompt
prompt = client.prompts.render(
    slug="customer-support-agent",
    label="production",
    variables={"user_name": "Alice", "question": "Help me"}
)

# Call LLM
response = client.chat.completions.create(**prompt)
```

### Dashboard Integration

The TrueFlow Dashboard provides a UI for:

- Browsing prompts by folder
- Editing prompt versions
- Deploying to labels
- Viewing version history

---

## Best Practices

### 1. Use Meaningful Slugs

```json
{ "slug": "customer-support-agent-v2" }  // Good
{ "slug": "prompt-123" }                  // Bad
```

### 2. Write Commit Messages

```json
{ "commit_message": "Reduced verbosity and added tone guidelines" }  // Good
{ "commit_message": "update" }                                         // Bad
```

### 3. Use Folders for Organization

```
/customer-support/ticket-response
/customer-support/escalation
/sales/lead-qualification
/engineering/code-review
```

### 4. Test in Staging First

```bash
# Deploy to staging
curl -X POST .../deploy -d '{"version": 5, "label": "staging"}'

# Test
curl .../render?label=staging&...

# Promote to production
curl -X POST .../deploy -d '{"version": 5, "label": "production"}'
```

### 5. Keep Versions Small

Make incremental changes rather than large rewrites:

- Easier to review changes
- Simpler rollback if needed
- Better audit trail

### 6. Use Variables for Personalization

```json
{
  "messages": [
    {
      "role": "system",
      "content": "You are helping {{user_name}} from {{company_name}}. Their account tier is {{tier}}."
    }
  ]
}
```

Variables allow the same prompt template to serve different contexts.

---

## Example Workflows

### Customer Support Bot

```bash
# Create prompt
curl -X POST .../prompts -d '{
  "name": "Ticket Response",
  "slug": "ticket-response",
  "folder": "/support"
}'

# Version 1
curl -X POST .../versions -d '{
  "model": "gpt-4o",
  "messages": [
    {"role": "system", "content": "You are a support agent. Respond to the customer."},
    {"role": "user", "content": "{{ticket_content}}"}
  ],
  "commit_message": "Initial version"
}'

# Version 2 (improved)
curl -X POST .../versions -d '{
  "model": "gpt-4o",
  "messages": [
    {"role": "system", "content": "You are a support agent for {{company_name}}. Be helpful and empathetic. Sign off as {{agent_name}}."},
    {"role": "user", "content": "{{ticket_content}}"}
  ],
  "commit_message": "Added personalization and tone"
}'

# Deploy version 2 to staging
curl -X POST .../deploy -d '{"version": 2, "label": "staging"}'

# Test, then deploy to production
curl -X POST .../deploy -d '{"version": 2, "label": "production"}'
```

### Code Review Assistant

```bash
# Create prompt
curl -X POST .../prompts -d '{
  "name": "Code Review",
  "slug": "code-review",
  "folder": "/engineering"
}'

# Version 1
curl -X POST .../versions -d '{
  "model": "claude-3-5-sonnet-20241022",
  "messages": [
    {"role": "system", "content": "Review the following code for bugs, security issues, and improvements:"},
    {"role": "user", "content": "{{code_diff}}"}
  ],
  "commit_message": "Initial code review prompt"
}'
```