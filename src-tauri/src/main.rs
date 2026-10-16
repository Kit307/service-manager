fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "proxy") {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Unable to start proxy runtime");
        std::process::exit(runtime.block_on(service_manager::proxy::run(
            args.into_iter().skip(1).collect(),
        )));
    }
    #[cfg(feature = "desktop")]
    service_manager::desktop::run();
}
