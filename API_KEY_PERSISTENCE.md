# API Key Persistence - Implementation Details

## How API Keys Are Stored

API keys are **securely stored in Windows Credential Manager** (the OS keychain) and are **fully persistent** across app restarts.

### Storage Flow

1. **When Creating a Provider:**
   - User enters API key in the form
   - Key is sent to backend
   - Backend stores key in Windows Credential Manager with a unique reference ID
   - Reference ID (auth_ref) is stored in SQLite database
   - **The actual API key is NEVER stored in the database**

2. **When Updating a Provider:**
   - If user enters a new API key: it updates the existing keychain entry
   - If user leaves API key field empty: the existing key is preserved
   - The keychain entry persists even if you close and reopen the app

3. **When Using a Provider:**
   - Backend retrieves the auth_ref from database
   - Uses auth_ref to fetch the actual API key from Windows Credential Manager
   - API key is used for the request, then discarded from memory

### Security Features

✅ **API keys stored in OS keychain** - Windows Credential Manager (most secure)
✅ **Never stored in database** - Only a reference ID is stored
✅ **Never displayed in UI** - Keys are masked and never shown back to user
✅ **Persistent across restarts** - Keys remain in keychain until explicitly deleted
✅ **Automatic cleanup** - When provider is deleted, keychain entry is also removed

### Verification

You can verify your API keys are stored by:
1. Creating a provider with an API key
2. Closing and reopening the app
3. Testing the connection - it should work without re-entering the key

### Troubleshooting

If API keys don't persist:
- Check Windows Credential Manager: Open `credential manager` in Windows
- Look for entries under "brain-stormer" service
- Ensure the app has permission to access Credential Manager
- Try deleting and recreating the provider

### Technical Details

- **Service Name**: `brain-stormer`
- **Username Format**: `provider_{provider_id}`
- **Storage Location**: Windows Credential Manager (encrypted by OS)
- **Database**: Only stores `auth_ref` (reference ID), never the actual key
