# Authentication Flow Analysis for RolX Angular Frontend

## 1. Google Sign-In Library Used

- Library: Google Identity Services (GIS) - the newer accounts.google.com library
- Type: google-one-tap (TypeScript types: @types/google-one-tap v1.2.2)
- Global object: google.accounts.id API
- Script loading: Loaded via <script src="https://accounts.google.com/gsi/client" defer></script> in index.html (line 18)

## 2. Sign-In Mode

Popup mode with One-Tap fallback:
- Uses google.accounts.id.renderButton() to create a styled sign-in button
- Also calls google.accounts.id.prompt() to display the One-Tap dialog (GoogleAuthService.ts, line 50)
- Mode: Popup for the button; One-Tap prompt as a secondary UX layer

## 3. Redirect URIs/Origins Referenced

No explicit redirect URIs found in the codebase. The frontend:
- Uses popup/One-Tap mode (no redirects needed)
- Loads the Google client ID from backend API at runtime via /api/v1/signin/info
- The backend must be configured with the allowed origins in Google Cloud Console

Relevant files/lines:
- /home/roger/dev/rolx/RolX/source/rolx-client/src/app/auth/core/info.ts - defines the Info interface with googleClientId
- /home/roger/dev/rolx/RolX/source/rolx-client/src/app/auth/core/google-auth.service.ts line 33-36 - fetches googleClientId from backend and initializes

## 4. JWT/Bearer Token Storage

localStorage with key name: currentApproval

Structure (stored as JSON):
{
  "user": { ... },
  "bearerToken": "string (JWT)",
  "expires": "ISO DateTime string"
}

Details:
- File: /home/roger/dev/rolx/RolX/source/rolx-client/src/app/auth/core/auth.service.ts
- Storage method: localStorage.setItem('currentApproval', JSON.stringify(instanceToPlain(approval))) (line 112)
- Retrieved on app init: localStorage.getItem('currentApproval') (line 93)
- Cleared on logout: localStorage.removeItem('currentApproval') (line 116)

## 5. Complete Sign-In Flow

File locations:
- /home/roger/dev/rolx/RolX/source/rolx-client/src/app/main/sign-in-page/sign-in-page.component.ts - main component
- /home/roger/dev/rolx/RolX/source/rolx-client/src/app/auth/core/google-auth.service.ts - Google integration
- /home/roger/dev/rolx/RolX/source/rolx-client/src/app/auth/core/auth.service.ts - token management
- /home/roger/dev/rolx/RolX/source/rolx-client/src/app/auth/core/auth.interceptor.ts - HTTP bearer token injection

Flow steps:

1. Initialize Google SDK (GoogleAuthService.initialize(), line 26-42):
  - Fetches backend info: GET /api/v1/signin/info → receives googleClientId and installationId
  - Calls google.accounts.id.initialize({client_id, callback})
2. Render UI (SignInPageComponent.ngAfterViewInit(), line 42-48):
  - Renders Google Sign-In button in template div #signInButton
  - Renders One-Tap prompt
3. User clicks button or One-Tap trigger:
  - Google returns CredentialResponse with credential (ID token/JWT from Google)
  - Callback: propagateCredentials() (line 53-55) emits credential via RxJS Subject
4. Handle credential (SignInPageComponent line 38):
  - Subscribes to googleAuthService.$credentials
  - Calls signIn(googleIdToken) with the Google ID token
5. Backend validation & JWT generation (signIn() method, line 54-70):
  - Sends POST /api/v1/signin with {googleIdToken}
  - Backend validates Google token and returns Approval object
  - Error handling: 401 status shows "Access denied" error
6. Store JWT (AuthService.signIn(), line 77-85):
  - Calls signInService.signIn() to POST to backend
  - Receives Approval object containing:
      - user (User object with role)
    - bearerToken (JWT string)
    - expires (expiration datetime)
  - Stores in localStorage via setCurrentApproval() (line 111-113)
7. Use token in requests (AuthInterceptor.intercept(), line 22-39):
  - All HTTP requests automatically include: Authorization: Bearer ${approval.bearerToken} header
  - On 401 response: sign out and redirect to /sign-in
8. Token refresh (AuthService constructor, line 38-54):
  - Every 5 minutes: calls GET /api/v1/signin/extend to refresh token
  - Backend returns updated Approval object
  - In production, failed extension triggers sign-out and redirect

Key files:
- /home/roger/dev/rolx/RolX/source/rolx-client/src/app/auth/core/approval.ts - token data structure
- /home/roger/dev/rolx/RolX/source/rolx-client/src/app/auth/core/sign-in.data.ts - request structure ({googleIdToken})
