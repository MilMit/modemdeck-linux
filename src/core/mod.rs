pub mod power;
pub mod profiles;
pub mod runtime_status;
pub mod safety;
pub mod settings;

pub fn i18n_init_helper() {
    crate::i18n::init(&settings::Settings::load().language);
}

pub fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    let rt = RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Tokio runtime failed")
    });
    rt.block_on(fut)
}
