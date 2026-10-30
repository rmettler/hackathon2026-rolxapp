use dioxus::prelude::*;
use rolx_core::session::Session;
use rolx_core::store::StoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Login,
    Day,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteEvent {
    SignedIn,
    SignedOut,
    /// The server does not accept the session any more.
    SessionRejected,
    OpenSettings,
    CloseSettings {
        has_session: bool,
    },
}

/// The first screen and the session to start with: the day if a usable session is stored, otherwise
/// the login without a session. A store that cannot be read counts as no session, the user can
/// simply sign in again.
pub fn start(stored: Result<Option<Session>, StoreError>) -> (Route, Option<Session>) {
    match stored {
        Ok(Some(session)) if !session.is_expired() => (Route::Day, Some(session)),
        _ => (Route::Login, None),
    }
}

pub fn next_route(route: Route, event: RouteEvent) -> Route {
    match event {
        RouteEvent::SignedIn => Route::Day,
        RouteEvent::SignedOut | RouteEvent::SessionRejected => Route::Login,
        RouteEvent::OpenSettings => Route::Settings,
        RouteEvent::CloseSettings { has_session } if route == Route::Settings => {
            if has_session {
                Route::Day
            } else {
                Route::Login
            }
        }
        RouteEvent::CloseSettings { .. } => route,
    }
}

/// App-wide state, provided as a context by [`crate::App`].
#[derive(Clone, Copy)]
pub struct AppState {
    pub route: Signal<Route>,
    pub session: Signal<Option<Session>>,
}

impl AppState {
    /// Follows `event`. Leaving the session behind on the way out is not left to the caller, so
    /// the settings screen cannot return to a day without a session.
    pub fn navigate(mut self, event: RouteEvent) {
        if matches!(event, RouteEvent::SignedOut | RouteEvent::SessionRejected) {
            self.session.set(None);
        }
        let next = next_route(*self.route.peek(), event);
        self.route.set(next);
    }

    pub fn close_settings(self) {
        let has_session = self.session.peek().is_some();
        self.navigate(RouteEvent::CloseSettings { has_session });
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use chrono::Duration;
    use rolx_core::clock;
    use rolx_core::session::BearerToken;

    use super::*;

    pub(crate) fn session_expiring_in(lifetime: Duration) -> Session {
        Session {
            bearer_token: BearerToken::from_string("fake".into()),
            expires: clock::now() + lifetime,
            user_id: "id".into(),
            user_email: "test@example.invalid".into(),
            user_first_name: "Test".into(),
            user_last_name: "User".into(),
        }
    }

    #[test]
    fn starts_at_the_day_with_a_usable_session() {
        let (route, session) = start(Ok(Some(session_expiring_in(Duration::hours(1)))));
        assert_eq!(route, Route::Day);
        assert!(session.is_some());

        let (route, _) = start(Ok(Some(session_expiring_in(Duration::minutes(1)))));
        assert_eq!(route, Route::Day);
    }

    #[test]
    fn starts_at_the_login_without_a_session_when_none_is_usable() {
        let corrupt = serde_json::from_str::<Session>("not json").unwrap_err();
        let stored = [
            Ok(None),
            Ok(Some(session_expiring_in(Duration::seconds(-1)))),
            Err(StoreError::CacheDirNotFound),
            Err(StoreError::Format(corrupt)),
        ];
        for stored in stored {
            let (route, session) = start(stored);
            assert_eq!(route, Route::Login);
            assert!(session.is_none(), "an expired session must not be kept");
        }
    }

    #[test]
    fn signing_in_and_out_switch_between_login_and_day() {
        assert_eq!(next_route(Route::Login, RouteEvent::SignedIn), Route::Day);
        assert_eq!(next_route(Route::Day, RouteEvent::SignedOut), Route::Login);
        assert_eq!(
            next_route(Route::Day, RouteEvent::SessionRejected),
            Route::Login
        );
        assert_eq!(
            next_route(Route::Settings, RouteEvent::SessionRejected),
            Route::Login
        );
    }

    #[test]
    fn closing_settings_returns_according_to_the_session() {
        let close = |has_session| RouteEvent::CloseSettings { has_session };

        assert_eq!(
            next_route(Route::Day, RouteEvent::OpenSettings),
            Route::Settings
        );
        assert_eq!(next_route(Route::Settings, close(true)), Route::Day);
        assert_eq!(next_route(Route::Settings, close(false)), Route::Login);
        assert_eq!(next_route(Route::Login, close(true)), Route::Login);
    }

    /// Runs `check` where signals can be created.
    fn in_runtime(check: fn()) {
        fn host(check: fn()) -> Element {
            use_hook(check);
            rsx! {}
        }

        VirtualDom::new_with_props(host, check).rebuild_in_place();
    }

    fn state_on_settings_with_a_session() -> AppState {
        AppState {
            route: Signal::new(Route::Settings),
            session: Signal::new(Some(session_expiring_in(Duration::hours(1)))),
        }
    }

    #[test]
    fn closing_settings_with_a_session_returns_to_the_day() {
        in_runtime(|| {
            let state = state_on_settings_with_a_session();
            state.close_settings();
            assert_eq!(*state.route.peek(), Route::Day);
        });
    }

    #[test]
    fn signing_out_or_a_rejected_session_clears_the_session() {
        in_runtime(|| {
            for event in [RouteEvent::SignedOut, RouteEvent::SessionRejected] {
                let state = state_on_settings_with_a_session();
                state.navigate(event);
                assert_eq!(*state.route.peek(), Route::Login);
                assert!(state.session.peek().is_none());
            }
        });
    }
}
