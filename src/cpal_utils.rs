use anyhow::{bail, Result};
use cpal::{
    self, SampleFormat, SampleRate, StreamConfig, SupportedInputConfigs, SupportedOutputConfigs,
};

// I'm sure there's a way to make this generic, but..
pub fn find_input_stream_config(
    supported_configs: SupportedInputConfigs,
    channels: u16,
    sample_rate: u32,
) -> Result<StreamConfig> {
    let cpal_sample_rate = SampleRate(sample_rate);
    for supported_config in supported_configs {
        if supported_config.sample_format() != SampleFormat::F32
            || supported_config.channels() != channels
            || supported_config.min_sample_rate() > cpal_sample_rate
            || supported_config.max_sample_rate() < cpal_sample_rate
        {
            continue;
        }
        return Ok(supported_config.with_sample_rate(cpal_sample_rate).into());
    }
    bail!("Failed to find matching stream config.");
}

pub fn find_output_stream_config(
    supported_configs: SupportedOutputConfigs,
    channels: u16,
    sample_rate: u32,
) -> Result<StreamConfig> {
    let cpal_sample_rate = SampleRate(sample_rate);
    for supported_config in supported_configs {
        if supported_config.sample_format() != SampleFormat::F32
            || supported_config.channels() != channels
            || supported_config.min_sample_rate() > cpal_sample_rate
            || supported_config.max_sample_rate() < cpal_sample_rate
        {
            continue;
        }
        return Ok(supported_config.with_sample_rate(cpal_sample_rate).into());
    }
    bail!("Failed to find matching stream config.");
}
