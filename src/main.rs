use std::sync::Arc;

use zitie::config::Config;
use zitie::web::{router, AppState};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(run())
}

async fn run() -> anyhow::Result<()> {
    let cfg = Arc::new(Config::from_env());
    std::fs::create_dir_all(&cfg.data_dir)?;
    std::fs::create_dir_all(cfg.fonts_dir())?;
    std::fs::create_dir_all(cfg.cache_dir())?;

    // 数据库
    let db = zitie::db::init(&cfg.db_path()).await?;

    // 字体
    let font_store = Arc::new(zitie::fonts::FontStore::load_builtin(&cfg.builtin_fonts_dir())?);
    font_store.load_uploads(&db, &cfg.fonts_dir()).await?;
    tracing::info!(count = font_store.list().len(), "fonts loaded");

    // PDF 缓存 + 周期清理
    let cache = Arc::new(zitie::pdfcache::PdfCache::new(cfg.cache_dir(), cfg.cache_quota_mb)?);
    {
        let cache = cache.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(600));
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tick.tick().await;
                let cache = cache.clone();
                let _ = tokio::task::spawn_blocking(move || cache.cleanup_if_over_quota()).await;
            }
        });
    }

    let state = Arc::new(AppState {
        db,
        fonts: font_store,
        cache,
        cfg: cfg.clone(),
    });
    let app = router(state);

    let listener = tokio::net::TcpListener::bind(&cfg.addr).await?;
    tracing::info!(addr = %cfg.addr, "zitie 字帖生成器已启动");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                term.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("收到退出信号,正在关闭");
}
