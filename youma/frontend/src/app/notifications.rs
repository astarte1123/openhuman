/// Notifications page.
///
/// A placeholder page that signals "coming soon" while still wiring into the
/// global state so future work can drop a `notifications` field into
/// `AppState` and render it here without restructuring the component.
use yew::prelude::*;

use super::state::AppCtx;

// ---------------------------------------------------------------------------
// Notifications page component
// ---------------------------------------------------------------------------

#[function_component(NotificationsPage)]
pub fn notifications_page() -> Html {
    // Read global state so we can surface a count badge once the field lands.
    // The state is already cloned cheaply (it's behind an Rc).
    let ctx = use_context::<AppCtx>().expect("[notifications] AppCtx missing from tree");

    // `AppState` does not yet carry a `notifications` vec, but we still access
    // the context to keep the component correctly wired into the provider tree.
    // The suppression is intentional — the binding will be used once the field
    // is added to `AppState`.
    let _ = &*ctx;

    // Placeholder count: 0 until the state field is added.
    let notification_count: usize = 0;

    html! {
        <section class="page-card notifications-page">
            <div class="notifications-header">
                <h2>
                    {"Notifications"}
                    if notification_count > 0 {
                        <span class="notifications-badge">{ notification_count }</span>
                    }
                </h2>
            </div>

            <div class="notifications-coming-soon">
                <div class="notifications-coming-soon__icon">{"\u{1f514}"}</div>
                <p class="notifications-coming-soon__title">{"Notifications coming soon"}</p>
                <p class="notifications-coming-soon__body">
                    {"Real-time notifications from your connected channels and agents \
                    will appear here once the feature ships."}
                </p>
            </div>
        </section>
    }
}
