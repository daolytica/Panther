# Provider Base URLs Guide

## Base URL Requirements

### OpenAI-Compatible Providers
**Base URL is OPTIONAL** - if left empty, defaults to `https://api.openai.com/v1`

#### Common OpenAI-Compatible Services:

| Service | Base URL | Notes |
|---------|----------|-------|
| **OpenAI (Official)** | `https://api.openai.com/v1` | Default - can leave empty |
| **OpenRouter** | `https://openrouter.ai/api/v1` | Multi-model API gateway |
| **Together AI** | `https://api.together.xyz/v1` | Open source models |
| **Anyscale** | `https://api.endpoints.anyscale.com/v1` | Open source models |
| **Groq** | `https://api.groq.com/openai/v1` | Fast inference |
| **Perplexity** | `https://api.perplexity.ai` | Search-enhanced models |
| **DeepInfra** | `https://api.deepinfra.com/v1/openai` | Cost-effective inference |
| **Local OpenAI-compatible** | `http://localhost:8000/v1` | Your own server (varies) |

### Local HTTP Providers
**Base URL is REQUIRED**

| Service | Base URL | Default Port | Notes |
|---------|----------|--------------|-------|
| **Ollama** | `http://localhost:11434` | 11434 | Most common local setup |
| **LM Studio** | `http://localhost:1234/v1` | 1234 | OpenAI-compatible API |
| **llama.cpp server** | `http://localhost:8080` | 8080 | Varies by config |
| **vLLM** | `http://localhost:8000/v1` | 8000 | Production local server |
| **Text Generation WebUI** | `http://localhost:5000` | 5000 | Varies by config |

### Anthropic (Claude)
**Base URL is OPTIONAL** - if left empty, defaults to `https://api.anthropic.com`
- Base URL: `https://api.anthropic.com` (default, without /v1)
- Note: The `/v1` is part of the endpoint path, not the base URL
- Authentication: Uses `x-api-key` header (not Bearer token)
- API Key: Get from https://console.anthropic.com/

### Google/Gemini
**Base URL is OPTIONAL** - if left empty, defaults to `https://generativelanguage.googleapis.com/v1`
- Base URL: `https://generativelanguage.googleapis.com/v1` (default)
- Authentication: API key as query parameter or header
- API Key: Get from https://makersuite.google.com/app/apikey

## Quick Reference

### For OpenAI-Compatible:
- **Leave empty** = Uses `https://api.openai.com/v1` (OpenAI official)
- **Custom URL** = Use for other OpenAI-compatible services

### For Local HTTP:
- **Ollama**: `http://localhost:11434`
- **LM Studio**: `http://localhost:1234/v1`
- **Custom**: Check your local server's documentation

## Examples

### Example 1: OpenAI Official
```
Provider Type: OpenAI Compatible
Base URL: (leave empty or use https://api.openai.com/v1)
API Key: sk-...
```

### Example 2: OpenRouter
```
Provider Type: OpenAI Compatible
Base URL: https://openrouter.ai/api/v1
API Key: sk-or-...
```

### Example 3: Ollama (Local)
```
Provider Type: Local HTTP
Base URL: http://localhost:11434
API Key: (not required)
```

### Example 4: LM Studio (Local)
```
Provider Type: Local HTTP
Base URL: http://localhost:1234/v1
API Key: (not required)
```

## Testing Your Base URL

After adding a provider, use the **Test** button to verify:
- The base URL is correct
- The service is accessible
- Your API key works (for cloud providers)
- Your local server is running (for local providers)

## Troubleshooting

**"Connection failed" errors:**
- Check if the base URL is correct
- For local providers, ensure the service is running
- For cloud providers, verify your API key
- Check your network/firewall settings

**Local provider not working:**
- Ensure the local server is running
- Check the port number matches
- Try `http://localhost:PORT` in your browser to verify it's accessible
