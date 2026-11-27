//! System sleep/resume listener using pure-Rust async zbus on Linux logind.

#[cfg(target_os = "linux")]
pub fn watch_prepare_for_sleep<F>(callback: F) -> Result<(), String>
where
    F: Fn(bool) + Send + Sync + 'static,
{
    std::thread::spawn(move || {
        crate::core::block_on(async move {
            let Ok(conn) = zbus::Connection::system().await else {
                return;
            };
            let Ok(proxy) = zbus::Proxy::new(
                &conn,
                "org.freedesktop.login1",
                "/org/freedesktop/login1",
                "org.freedesktop.login1.Manager",
            )
            .await
            else {
                return;
            };
            let Ok(mut stream) = proxy.receive_signal("PrepareForSleep").await else {
                return;
            };
            use std::pin::Pin;
            use zbus::export::futures_core::Stream;
            while let Some(msg) =
                std::future::poll_fn(|cx| Pin::new(&mut stream).poll_next(cx)).await
            {
                if let Ok(sleeping) = msg.body().deserialize::<bool>() {
                    callback(sleeping);
                }
            }
        });
    });
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn watch_prepare_for_sleep<F>(_callback: F) -> Result<(), String>
where
    F: Fn(bool) + Send + Sync + 'static,
{
    Ok(())
}
