use chrono::offset::FixedOffset;
use simplelog::*;

pub fn setup_logging() {
    let config = ConfigBuilder::new()
        .set_time_level(LevelFilter::Error)
        .set_location_level(LevelFilter::Error)
        .set_target_level(LevelFilter::Error)
        .set_location_level(LevelFilter::Error)
        .set_time_offset(FixedOffset::west(5))
        .build();
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Debug,
        config,
        TerminalMode::Stdout,
    )
    .unwrap()])
    .unwrap();
}
