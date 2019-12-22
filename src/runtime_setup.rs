use simplelog::*;

pub fn setup_logging() {
    let config = ConfigBuilder::new()
        .set_time_level(LevelFilter::Error)
        .set_location_level(LevelFilter::Error)
        .set_target_level(LevelFilter::Error)
        .set_location_level(LevelFilter::Error)
        .build();
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Debug,
        config,
        TerminalMode::Stdout,
    )
    .unwrap()])
    .unwrap();
}
