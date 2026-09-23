use std::rc::Rc;

/// Subscribe to systemd-logind's PrepareForSleep signal. The callback gets
/// true immediately before suspend and false after resume.
#[allow(deprecated)]
pub fn watch_prepare_for_sleep<F>(callback: F) -> Result<gio::SignalSubscriptionId, glib::Error>
where
    F: Fn(bool) + 'static,
{
    let connection = gio::bus_get_sync(gio::BusType::System, None::<&gio::Cancellable>)?;
    let callback = Rc::new(callback);
    let cb = callback.clone();
    Ok(connection.signal_subscribe(
        Some("org.freedesktop.login1"),
        Some("org.freedesktop.login1.Manager"),
        Some("PrepareForSleep"),
        Some("/org/freedesktop/login1"),
        None,
        gio::DBusSignalFlags::NONE,
        move |_, _, _, _, _, parameters| {
            if parameters.n_children() > 0 {
                if let Some(sleeping) = parameters.child_value(0).get::<bool>() {
                    cb(sleeping);
                }
            }
        },
    ))
}
