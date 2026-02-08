# Model Selection Guide

## Overview

When creating profiles (agents), you can:
1. **Select from available models** - Auto-detect models from your provider
2. **Manually enter model name** - Enter any model version you know
3. **Create multiple profiles with same provider** - Use one API key for multiple agents with different models

## How Model Selection Works

### Automatic Model Detection

1. **Select a Provider** - Choose the provider you want to use
2. **Click "Refresh Models"** - The app will fetch available models from that provider
3. **Select from Dropdown** - Choose the specific model version you want

### Manual Model Entry

If auto-detection doesn't work or you want a specific version:
1. Click **"Manual Entry"** button
2. Type the model name directly
3. Examples:
   - `gpt-4`
   - `gpt-4-turbo-preview`
   - `gpt-3.5-turbo`
   - `claude-3-opus-20240229`
   - `llama-3-70b-instruct`

### Creating Multiple Agents with Same Provider

**Example Scenario:**
- You have one OpenAI API key
- You want to create 3 different agents:
  1. **Architect** - using `gpt-4` (more capable, slower)
  2. **Critic** - using `gpt-4-turbo` (faster, still capable)
  3. **PM** - using `gpt-3.5-turbo` (fastest, cost-effective)

**How to do it:**
1. Create Profile 1:
   - Provider: Your OpenAI provider
   - Model: `gpt-4`
   - Persona: Architect
   
2. Create Profile 2:
   - Provider: **Same** OpenAI provider (same API key)
   - Model: `gpt-4-turbo` (different model!)
   - Persona: Critic
   
3. Create Profile 3:
   - Provider: **Same** OpenAI provider
   - Model: `gpt-3.5-turbo` (different model!)
   - Persona: PM

All three profiles use the same provider/API key but different models!

## Common Model Names

### OpenAI
- `gpt-4`
- `gpt-4-turbo-preview`
- `gpt-4-0125-preview`
- `gpt-3.5-turbo`
- `gpt-3.5-turbo-16k`

### Anthropic (when implemented)
- `claude-3-opus-20240229`
- `claude-3-sonnet-20240229`
- `claude-3-haiku-20240307`

### Local Models (Ollama)
- `llama3`
- `llama3:70b`
- `mistral`
- `codellama`
- `phi`

### Local Models (LM Studio)
- Varies by what you have loaded
- Usually shows as model names you've downloaded

## Tips

1. **Use Refresh Models** - Always try refreshing first to see what's available
2. **Check Provider Documentation** - For exact model names if auto-detection fails
3. **Test Different Models** - Create multiple profiles to compare model performance
4. **Cost Optimization** - Use faster/cheaper models for simple tasks, powerful models for complex ones
