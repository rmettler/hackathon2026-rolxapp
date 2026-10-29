# RolX Server Authentication Implementation - Detailed Analysis

## 1. POST api/v1/SignIn Endpoint

**File:** `/home/roger/dev/rolx/RolX/source/RolXServer/RolXServer/Auth/WebApi/SignInController.cs`

### Accepts

- **Request body:** `SignInData` object containing:
  - `GoogleIdToken` (string) - the Google ID token from the client

### Returns

- **Success (HTTP 200):** Approval response object with:
  - `User` (User resource containing: Id, FirstName, LastName, Email, AvatarUrl, Role, EntryDate, LeavingDate, IsConfirmed, PartTimeSettings, BalanceCorrections, VacationDaysSettings, TargetBillabilitySettings, PositionSettings)
  - `BearerToken` (string) - JWT bearer token for subsequent authenticated requests
  - `Expires` (DateTime) - token expiration timestamp
- **Failure (HTTP 401 Unauthorized):** Empty response

---

## 2. Google ID Token Validation

**File:** `/home/roger/dev/rolx/RolX/source/RolXServer/RolXServer/Auth/Domain/Detail/SignInService.cs`

### Validation Flow (lines 75-104)

1. **Google JWT Signature Validation:**
   ```csharp
   var payload = await GoogleJsonWebSignature.ValidateAsync(signInData.GoogleIdToken);
   ```
   - Uses `Google.Apis.Auth` library (v1.67.0)
   - Validates JWT signature via Google's public keys (automatically downloaded)
   - **CRITICAL:** The code does NOT pass a second argument to `ValidateAsync()`, meaning it does NOT verify the `aud` (audience) claim against a specific client ID
   - Only validates signature and expiration

2. **Hosted Domain Whitelist Check (lines 85-89):**
   - Verifies `payload.HostedDomain` is in the whitelist configured at `settings.GoogleHostedDomainWhitelist`
   - If the hosted domain check fails, logs: `"Sign-in from foreign domain refused"`
   - Returns `null` (401 Unauthorized)

3. **User Confirmation Check (lines 91-95):**
   - User must exist in database and have `IsConfirmed = true`
   - First user is automatically confirmed; subsequent users require admin approval

4. **Exception Handling (lines 99-103):**
   - Catches `InvalidJwtException` from token validation failures
   - Returns `null` on any validation error

---

## 3. Approval Response Structure

**Domain Model:** `/home/roger/dev/rolx/RolX/source/RolXServer/RolXServer/Auth/Domain/Model/Approval.cs`

**WebApi Resource:** `/home/roger/dev/rolx/RolX/source/RolXServer/RolXServer/Auth/WebApi/Resource/Approval.cs`

Both contain identical fields:

```csharp
public class Approval
{
    public User User { get; set; }
    public string BearerToken { get; set; }
    public DateTime Expires { get; set; }
}
```

The domain model is mapped to the resource via `ApprovalMapper.ToResource()` before serialization.

---

## 4. Google Client ID Configuration & aud Field Validation

**Files:**
- `/home/roger/dev/rolx/RolX/source/RolXServer/RolXServer/Auth/Settings.cs`
- `/home/roger/dev/rolx/RolX/source/RolXServer/RolXServer/appsettings.Development.json`
- `/home/roger/dev/rolx/RolX/source/RolXServer/RolXServer/appsettings.json`

### Client ID Storage

- Stored in `Auth.GoogleClientId` configuration setting
- **Development:** `844762267541-hdsbqh9fj21na7k5fch0i2dm7vlkqjs9.apps.googleusercontent.com`
- **Production:** Must be set via environment variable or config override (not shown in `appsettings.Production.json`)

### aud Claim Verification

- **NOT PERFORMED** by the server
- The `GoogleJsonWebSignature.ValidateAsync()` call with a single argument only validates signature
- `Google.Apis.Auth` library supports passing a client ID as a second parameter to validate `aud`, but the code does not use it
- **Security Implication:** Any Google ID token signed by Google can be used, regardless of which client ID issued it (though the hosted domain whitelist provides a secondary control)

---

## 5. Allowed Origins & CORS Configuration

**File:** `/home/roger/dev/rolx/RolX/source/RolXServer/RolXServer/Program.cs` (lines 85-90)

### CORS Policy

```csharp
app.UseCors(builder => builder
    .WithOrigins("http://localhost:4200")
    .WithExposedHeaders("content-disposition")
    .AllowAnyHeader()
    .AllowAnyMethod());
```

- **Allowed Origin:** `http://localhost:4200` (Angular development server)
- **Exposed Headers:** `content-disposition`
- **Allowed Methods:** Any (GET, POST, PUT, DELETE, OPTIONS, etc.)
- **Allowed Headers:** Any

> **Note:** This is development-only configuration. No production CORS configuration is visible in the checked files.

---

## 6. API Key Authentication Path

**Files:**
- `/home/roger/dev/rolx/RolX/source/RolXServer/RolXServer/Auth/Domain/Detail/ApiKeyAuthenticationSchemeHandler.cs`
- `/home/roger/dev/rolx/RolX/source/RolXServer/RolXServer/ApiKeys/WebApi/ApiKeyController.cs`

### API Key Authentication Scheme

- **Header:** `X-API-KEY` (case-sensitive)

### Validation (lines 36-69 of ApiKeyAuthenticationSchemeHandler)

a. Extracts header value
b. Checks against environment variable-based static key: `Options.ApiKey`
c. Falls back to database validation via `IApiKeyValidator.IsValid(apiKey)`
d. Returns `AuthenticateResult.Fail("Invalid API Key")` if both fail

### Claims Issued When Valid (lines 44-49)

- `ClaimTypes.Name`: `"ApiKey"`
- `ClaimTypes.Role`: `"Supervisor"`
- `RolXClaimTypes.EntryDate`: Current date in ISO format

### Database Storage

API keys are stored hashed in the database (uses `IHashingService`)

### API Key Management Endpoints (`/api/v1/ApiKey`)

| Method | Endpoint | Requirements |
|--------|----------|--------------|
| POST   | `/api/v1/ApiKey` | Create new API key (requires Administrator role) |
| GET    | `/api/v1/ApiKey` | List all API keys (requires Administrator role) |
| GET    | `/api/v1/ApiKey/{id}` | Get specific API key (requires Administrator role) |
| PUT    | `/api/v1/ApiKey/{id}` | Update API key (requires Administrator role) |
| POST   | `/api/v1/ApiKey/{id}/rotate` | Rotate API key (requires Administrator role) |
| DELETE | `/api/v1/ApiKey/{id}` | Delete API key (requires Administrator role) |

---

## 7. Bearer Token Generation

**File:** `/home/roger/dev/rolx/RolX/source/RolXServer/RolXServer/Auth/Domain/Detail/BearerTokenFactory.cs`

### JWT Configuration

- **Algorithm:** HMAC-SHA256
- **Key:** Secret from `Auth.Secret` config (must be replaced for production)
- **Expiration:** Configurable via `Auth.TokenExpiration` (default: 1 day in production, 7 days in development)
- **Issuer Validation:** Disabled (`ValidateIssuer = false`)
- **Audience Validation:** Disabled (`ValidateAudience = false`)

### Claims in Generated Token

- `ClaimTypes.NameIdentifier`: User GUID
- `ClaimTypes.Role`: User role (Administrator, User, etc.)
- `RolXClaimTypes.EntryDate`: User entry date (ISO format)
- `RolXClaimTypes.LeftDate`: User left date (ISO format, only if set)

---

## 8. Configuration Settings Summary

**File:** `/home/roger/dev/rolx/RolX/source/RolXServer/RolXServer/Auth/Settings.cs`

```csharp
public class Settings
{
    public string Secret { get; set; }                              // JWT signing secret
    public TimeSpan TokenExpiration { get; set; }                   // Token lifetime
    public string GoogleClientId { get; set; }                      // For client-side use only
    public string[] GoogleHostedDomainWhitelist { get; set; }       // Allowed Google domains
    public string InstallationId { get; set; }                      // Identification string
    public string ApiKey { get; set; }                              // Static API key
}
```

### Production Whitelist (appsettings.Production.json)

- `m-f.ch`
- `cudos.ch`

---

## Summary of Security Posture

| Aspect | Implementation | Status |
|--------|----------------|--------|
| Google JWT Signature | Validated via `Google.Apis.Auth` | ✓ Implemented |
| Google `aud` claim validation | Not performed | ⚠️ Missing |
| Hosted domain whitelist | Enforced for production | ✓ Implemented |
| User confirmation requirement | Enforced (except first user) | ✓ Implemented |
| Bearer token signing | HMAC-SHA256 | ✓ Implemented |
| API Key support | Hashed in DB + optional static key | ✓ Implemented |
| CORS for OAuth redirect | Dev only (localhost:4200) | ⚠️ Limited |
